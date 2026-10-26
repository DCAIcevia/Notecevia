#![forbid(unsafe_code)]

//! 首页：画板列表、搜索、标签筛选、缩略图与导出入口。
//!
//! 首页只产出一个动作，由 app 执行真正的读写。这样界面层不用碰文件。

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use egui::{
    Align, Align2, Button, Color32, CornerRadius, FontId, Key, Label, Layout, Margin, Rect,
    RichText, Sense, Shape, TextEdit, TextWrapMode, TextureHandle, TextureOptions, Ui, Vec2, pos2,
    text::LayoutJob, text::TextFormat, vec2,
};

use crate::{strings, theme};

const ROW_RADIUS: u8 = 8;
const ROW_PAD_X: i8 = 12;
const ROW_PAD_Y: i8 = 8;
const ACTION_WIDTH: f32 = 26.0;
const CHIP_HEIGHT: f32 = 18.0;
const CHIP_RADIUS: u8 = 9;
const CHIP_PAD: f32 = 7.0;
const SEARCH_WIDTH: f32 = 240.0;
const PAGE_MARGIN_X: i8 = 24;
const PAGE_MARGIN_Y: i8 = 16;
const ROW_GAP: f32 = 2.0;
const JUST_NOW_SECS: i64 = 60;
const DAY_SECS: i64 = 86_400;
const THUMB_WIDTH: f32 = 56.0;
const THUMB_HEIGHT: f32 = 40.0;
const THUMB_RADIUS: u8 = 4;
const THUMB_GAP: f32 = 10.0;
/// 缩略图解码后最长边压到这个像素数，纹理内存才有上限。
const MAX_THUMBNAIL_SIDE: u32 = 128;
/// 纹理缓存条数上限；滚过很多画板时不会无限堆积。
const MAX_CACHED_THUMBNAILS: usize = 256;

/// 列表里的一张画板。
#[derive(Clone, Debug, PartialEq)]
pub struct BoardItem {
    pub id: u64,
    pub title: String,
    pub tags: Vec<String>,
    /// 搜索命中的正文片段；没搜索时为 None。
    pub snippet: Option<String>,
    /// 最后更新时间，Unix 秒。
    pub updated_at: i64,
    pub entry_count: usize,
}

/// 用户想让 app 做的事。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HomeAction {
    Create,
    /// 外部程序改过工作区，请 app 重建索引并回填列表。
    Refresh,
    Open(u64),
    Delete(u64),
    Rename(u64, String),
    SetTags(u64, Vec<String>),
    Export(u64, ExportKind),
    /// 搜索词变了，app 去查一遍再回填。
    Search(String),
}

/// 导出的两种格式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportKind {
    Container,
    Html,
}

/// 首页按需取缩略图的来源，返回 PNG 字节。
///
/// 做成 trait 而不是往 BoardItem 里塞字节：列表只保留解码后的纹理缓存，
/// 不把一万张图背在内存里。取不到就返回 None，界面画占位。
pub trait ThumbnailSource {
    fn thumbnail(&self, board_id: u64, updated_at: i64) -> Option<Vec<u8>>;
}

/// 不需要缩略图时的默认来源。
impl ThumbnailSource for () {
    fn thumbnail(&self, _board_id: u64, _updated_at: i64) -> Option<Vec<u8>> {
        None
    }
}

/// 首页状态：列表、搜索词、待确认的删除、正在编辑的标题。
pub struct HomeApp {
    boards: Vec<BoardItem>,
    state: HomeState,
    thumbnails: Box<dyn ThumbnailSource>,
    /// 按 (画板 id, 更新时间) 缓存解码后的纹理；None 表示取过但没图/解码失败。
    textures: HashMap<(u64, i64), Option<TextureHandle>>,
}

/// 跟列表内容无关的交互状态。
#[derive(Default)]
struct HomeState {
    search: String,
    /// 选中的标签筛选；空表示不筛，多选按交集。
    filter: Vec<String>,
    pending_delete: Option<u64>,
    editing: Option<(u64, String)>,
    status: Option<String>,
    /// 键盘选中的行；点行打开后不保留意义，只服务上下键与 Enter。
    selected: Option<u64>,
    /// 正在编辑的标签输入框内容。
    tag_draft: String,
    /// 刚进入重命名时把焦点给输入框。
    rename_focus: bool,
    /// 标签编辑器刚打开时把焦点给输入框。
    tag_focus: bool,
    /// 这一帧有文本框拿着焦点，方向键留给光标。
    typing: bool,
    /// 键盘换了选中行，下一帧把它滚进视野。
    scroll_to: Option<u64>,
}

impl HomeApp {
    pub fn new() -> Self {
        Self::with_thumbnails(Box::new(()))
    }

    /// 用给定的缩略图来源建首页；new 用不提供缩略图的默认来源。
    pub fn with_thumbnails(source: Box<dyn ThumbnailSource>) -> Self {
        Self {
            boards: Vec::new(),
            state: HomeState::default(),
            thumbnails: source,
            textures: HashMap::new(),
        }
    }

    pub fn set_boards(&mut self, boards: Vec<BoardItem>) {
        self.boards = boards;
        let ids: Vec<u64> = self.boards.iter().map(|board| board.id).collect();
        let state = &mut self.state;
        if state.selected.is_some_and(|id| !ids.contains(&id)) {
            state.selected = None;
        }
        if state.pending_delete.is_some_and(|id| !ids.contains(&id)) {
            state.pending_delete = None;
        }
        if state
            .editing
            .as_ref()
            .is_some_and(|(id, _)| !ids.contains(id))
        {
            state.editing = None;
        }

        // 画板改名/更新后旧纹理不再对应，顺手丢掉；搜索换了结果集也会收窄。
        let latest: HashMap<u64, i64> = self
            .boards
            .iter()
            .map(|board| (board.id, board.updated_at))
            .collect();
        self.textures
            .retain(|&(id, updated_at), _| latest.get(&id) == Some(&updated_at));
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.state.status = Some(status.into());
    }

    pub fn board_count(&self) -> usize {
        self.boards.len()
    }

    /// 画一帧首页，返回用户这一帧想做的动作。
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<HomeAction> {
        let palette = theme::palette(ui.visuals().dark_mode);
        let mut action = None;
        let Self {
            boards,
            state,
            thumbnails,
            textures,
        } = self;
        state.typing = false;
        let mut thumbs = ThumbnailCache {
            source: thumbnails.as_ref(),
            textures,
        };
        let now = now_secs();

        egui::Frame::new()
            .inner_margin(Margin::symmetric(PAGE_MARGIN_X, PAGE_MARGIN_Y))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if let Some(chosen) = top_bar(state, ui, &palette) {
                    action = Some(chosen);
                }
                status_line(state, ui, &palette);
                filter_bar(state, ui, &palette, boards);
                ui.add_space(8.0);

                let visible = filtered_boards(boards, &state.filter);
                if state
                    .selected
                    .is_some_and(|id| !visible.iter().any(|board| board.id == id))
                {
                    state.selected = None;
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if visible.is_empty() {
                            if let Some(chosen) = placeholder(state, ui, &palette)
                                && action.is_none()
                            {
                                action = Some(chosen);
                            }
                            return;
                        }
                        for board in &visible {
                            if let Some(chosen) =
                                board_row(state, ui, &palette, board, now, &mut thumbs)
                                && action.is_none()
                            {
                                action = Some(chosen);
                            }
                            ui.add_space(ROW_GAP);
                        }
                    });

                handle_keys(state, &visible, ui, &mut action);
            });

        action
    }
}

impl Default for HomeApp {
    fn default() -> Self {
        Self::new()
    }
}

/// 顶部：标题、新建、搜索。
fn top_bar(state: &mut HomeState, ui: &mut Ui, palette: &theme::Palette) -> Option<HomeAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(strings::HOME_TITLE)
                .size(theme::TEXT_SIZES[3])
                .strong()
                .color(palette.text),
        );
        ui.add_space(12.0);
        if ui.button(strings::HOME_NEW).clicked() {
            action = Some(HomeAction::Create);
        }
        if ui.button(strings::HOME_REFRESH).clicked() {
            action = Some(HomeAction::Refresh);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if !state.search.is_empty() && ui.button("×").clicked() {
                state.search.clear();
                action = Some(HomeAction::Search(String::new()));
            }
            let response = ui.add(
                TextEdit::singleline(&mut state.search)
                    .hint_text(strings::HOME_SEARCH_HINT)
                    .desired_width(SEARCH_WIDTH),
            );
            state.typing |= response.has_focus();
            if response.changed() {
                action = Some(HomeAction::Search(state.search.clone()));
            }
        });
    });
    action
}

fn status_line(state: &HomeState, ui: &mut Ui, palette: &theme::Palette) {
    let Some(status) = state.status.as_deref() else {
        return;
    };
    ui.add_space(6.0);
    ui.label(
        RichText::new(status)
            .size(theme::SMALL_FONT)
            .color(palette.muted),
    );
}

/// 列表上方的一行标签筛选。多选按交集，再点一次取消。
fn filter_bar(state: &mut HomeState, ui: &mut Ui, palette: &theme::Palette, boards: &[BoardItem]) {
    let tags = all_tags(boards);
    if tags.is_empty() && state.filter.is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(
            RichText::new(strings::HOME_TAG_FILTER)
                .size(theme::SMALL_FONT)
                .color(palette.muted),
        );
        if filter_chip(
            ui,
            palette,
            strings::HOME_FILTER_ALL,
            state.filter.is_empty(),
        ) {
            state.filter.clear();
        }
        for tag in &tags {
            let active = state.filter.iter().any(|selected| selected == tag);
            if filter_chip(ui, palette, tag, active) {
                if active {
                    state.filter.retain(|selected| selected != tag);
                } else {
                    state.filter.push(tag.clone());
                }
            }
        }
        if !state.filter.is_empty() && filter_chip(ui, palette, strings::HOME_FILTER_CLEAR, false) {
            state.filter.clear();
        }
    });
    ui.add_space(4.0);
}

/// 可点的筛选标签片。active 时反色，表示这个标签正在生效。
fn filter_chip(ui: &mut Ui, palette: &theme::Palette, text: &str, active: bool) -> bool {
    let font = FontId::proportional(theme::SMALL_FONT);
    let color = if active {
        palette.canvas
    } else {
        palette.muted
    };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let size = vec2(galley.size().x + CHIP_PAD * 2.0, CHIP_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let fill = if active {
        palette.text
    } else if response.hovered() {
        palette.hover_edge
    } else {
        palette.block_fill
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(CHIP_RADIUS), fill);
    let text_pos = pos2(
        rect.left() + CHIP_PAD,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(text_pos, galley, color);
    response.clicked()
}

/// 所有画板标签按首次出现顺序去重汇总。
fn all_tags(boards: &[BoardItem]) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    for board in boards {
        for tag in &board.tags {
            if !tags.iter().any(|existing| existing == tag) {
                tags.push(tag.clone());
            }
        }
    }
    tags
}

/// 按选中的标签筛画板：多选取交集，空筛选全过。
fn filtered_boards<'a>(boards: &'a [BoardItem], filter: &[String]) -> Vec<&'a BoardItem> {
    boards
        .iter()
        .filter(|board| matches_filter(&board.tags, filter))
        .collect()
}

fn matches_filter(tags: &[String], filter: &[String]) -> bool {
    filter
        .iter()
        .all(|wanted| tags.iter().any(|tag| tag == wanted))
}

/// 空态与无结果态。两种情况都给下一步。
fn placeholder(state: &mut HomeState, ui: &mut Ui, palette: &theme::Palette) -> Option<HomeAction> {
    let searching = !state.search.trim().is_empty();
    let filtering = !state.filter.is_empty();
    let empty = !searching && !filtering;
    let mut action = None;
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        let (title, hint) = if empty {
            (strings::HOME_EMPTY, strings::HOME_EMPTY_HINT)
        } else {
            (strings::HOME_NO_RESULTS, strings::HOME_NO_RESULTS_HINT)
        };
        ui.label(
            RichText::new(title)
                .size(theme::TEXT_SIZES[3])
                .color(palette.text),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(hint)
                .size(theme::SMALL_FONT)
                .color(palette.muted),
        );
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            if empty && ui.button(strings::HOME_NEW).clicked() {
                action = Some(HomeAction::Create);
            }
            if filtering && ui.button(strings::HOME_FILTER_CLEAR).clicked() {
                state.filter.clear();
            }
            if searching && ui.button(strings::HOME_CLEAR_SEARCH).clicked() {
                state.search.clear();
                action = Some(HomeAction::Search(String::new()));
            }
        });
    });
    action
}

fn board_row(
    state: &mut HomeState,
    ui: &mut Ui,
    palette: &theme::Palette,
    board: &BoardItem,
    now: i64,
    thumbs: &mut ThumbnailCache<'_>,
) -> Option<HomeAction> {
    let selected = state.selected == Some(board.id);
    let confirming = state.pending_delete == Some(board.id);
    let editing_this = state
        .editing
        .as_ref()
        .is_some_and(|(id, _)| *id == board.id);
    let mut action = None;
    let mut typing = false;

    // 先占一个背景形状的位置，等内容排完再决定填什么色；这样底色不会盖住文字。
    let background = ui.painter().add(Shape::Noop);
    let response = ui
        .scope_builder(egui::UiBuilder::new().sense(Sense::click()), |ui| {
            egui::Frame::new()
                .inner_margin(Margin::symmetric(ROW_PAD_X, ROW_PAD_Y))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                        thumbnail_cell(ui, palette, thumbs, board);
                        ui.add_space(THUMB_GAP);
                        let content_width = (ui.available_width() - ACTION_WIDTH - 8.0).max(120.0);
                        ui.allocate_ui_with_layout(
                            vec2(content_width, 0.0),
                            Layout::top_down(Align::Min),
                            |ui| {
                                row_content(
                                    state,
                                    ui,
                                    palette,
                                    board,
                                    now,
                                    &mut action,
                                    &mut typing,
                                )
                            },
                        );
                        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                            let more = ui.add(Button::new("⋯").frame(false));
                            egui::Popup::from_toggle_button_response(&more)
                                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                                .show(|ui| board_menu(state, ui, board, &mut action));
                        });
                    });
                });
        })
        .response;
    state.typing |= typing;

    let rect = response.rect;
    if rect.is_positive() {
        if selected || response.contains_pointer() {
            ui.painter().set(
                background,
                Shape::rect_filled(rect, CornerRadius::same(ROW_RADIUS), palette.media_fill),
            );
        }
        // 键盘选中用左侧一道竖线标出来，不改几何、也不只靠颜色。
        if selected {
            let bar = Rect::from_min_max(
                pos2(rect.left() + 4.0, rect.top() + 8.0),
                pos2(rect.left() + 6.0, rect.bottom() - 8.0),
            );
            ui.painter()
                .rect_filled(bar, CornerRadius::same(1), palette.text);
        }
    }

    if action.is_none() && response.clicked() && !confirming && !editing_this {
        state.selected = Some(board.id);
        state.pending_delete = None;
        action = Some(HomeAction::Open(board.id));
    }
    response.context_menu(|ui| board_menu(state, ui, board, &mut action));

    if selected && state.scroll_to == Some(board.id) {
        response.scroll_to_me(Some(Align::Center));
        state.scroll_to = None;
    }

    action
}

/// 只保留已解码纹理的缩略图缓存；来源按 (id, updated_at) 取字节。
struct ThumbnailCache<'a> {
    source: &'a dyn ThumbnailSource,
    textures: &'a mut HashMap<(u64, i64), Option<TextureHandle>>,
}

impl ThumbnailCache<'_> {
    fn texture(&mut self, ctx: &egui::Context, board: &BoardItem) -> Option<TextureHandle> {
        let key = (board.id, board.updated_at);
        if let Some(cached) = self.textures.get(&key) {
            return cached.clone();
        }
        let texture = self
            .source
            .thumbnail(board.id, board.updated_at)
            .and_then(|bytes| rgba_thumbnail(&bytes))
            .map(|(size, rgba)| {
                let image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
                ctx.load_texture(
                    format!("home-thumb-{}-{}", board.id, board.updated_at),
                    image,
                    TextureOptions::LINEAR,
                )
            });
        // 没图或解码失败也记一条，避免每帧重试。
        if texture.is_some() && self.textures.len() >= MAX_CACHED_THUMBNAILS {
            let stale = self.textures.keys().find(|cached| **cached != key).copied();
            if let Some(stale) = stale {
                self.textures.remove(&stale);
            }
        }
        self.textures.insert(key, texture.clone());
        texture
    }
}

/// 行首缩略图。没图、解码失败或不在视野内都画占位，不阻塞列表。
fn thumbnail_cell(
    ui: &mut Ui,
    palette: &theme::Palette,
    thumbs: &mut ThumbnailCache<'_>,
    board: &BoardItem,
) {
    let (rect, _) = ui.allocate_exact_size(vec2(THUMB_WIDTH, THUMB_HEIGHT), Sense::hover());
    // 不在视野里的行不取图、不解码，滚到哪才加载哪。
    if !ui.is_rect_visible(rect) {
        return;
    }
    let radius = CornerRadius::same(THUMB_RADIUS);
    ui.painter().rect_filled(rect, radius, palette.media_fill);
    let Some(texture) = thumbs.texture(ui.ctx(), board) else {
        return;
    };
    let uv = cover_uv(texture.size_vec2(), rect.size());
    ui.painter().image(texture.id(), rect, uv, Color32::WHITE);
}

/// 解码 PNG，并把最长边压到 MAX_THUMBNAIL_SIDE，纹理内存才有上限。
fn rgba_thumbnail(bytes: &[u8]) -> Option<([usize; 2], Vec<u8>)> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    let image = if image.width() > MAX_THUMBNAIL_SIDE || image.height() > MAX_THUMBNAIL_SIDE {
        image.thumbnail(MAX_THUMBNAIL_SIDE, MAX_THUMBNAIL_SIDE)
    } else {
        image
    };
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    if size[0] == 0 || size[1] == 0 {
        return None;
    }
    Some((size, rgba.as_raw().clone()))
}

/// 按 cover 方式裁 UV：图比框宽就裁左右，比框高就裁上下。
fn cover_uv(image: Vec2, target: Vec2) -> Rect {
    let image_aspect = image.x / image.y;
    let target_aspect = target.x / target.y;
    if image_aspect > target_aspect {
        let width = target_aspect / image_aspect;
        Rect::from_min_max(
            pos2((1.0 - width) / 2.0, 0.0),
            pos2((1.0 + width) / 2.0, 1.0),
        )
    } else {
        let height = image_aspect / target_aspect;
        Rect::from_min_max(
            pos2(0.0, (1.0 - height) / 2.0),
            pos2(1.0, (1.0 + height) / 2.0),
        )
    }
}

fn row_content(
    state: &mut HomeState,
    ui: &mut Ui,
    palette: &theme::Palette,
    board: &BoardItem,
    now: i64,
    action: &mut Option<HomeAction>,
    typing: &mut bool,
) {
    let renaming = state
        .editing
        .as_ref()
        .is_some_and(|(id, _)| *id == board.id);
    if renaming {
        rename_field(state, ui, board, action, typing);
    } else {
        let title = if board.title.trim().is_empty() {
            strings::HOME_UNTITLED
        } else {
            board.title.as_str()
        };
        ui.add(
            Label::new(
                RichText::new(title)
                    .size(theme::BLOCK_FONT)
                    .color(palette.text),
            )
            .truncate(),
        );
    }

    if let Some(snippet) = board
        .snippet
        .as_deref()
        .filter(|snippet| !snippet.is_empty())
    {
        ui.add(
            Label::new(snippet_job(snippet, state.search.trim(), palette))
                .wrap_mode(TextWrapMode::Truncate),
        );
    }

    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.label(meta_text(board, now, palette));
        for tag in &board.tags {
            let _ = tag_chip(ui, palette, tag, false);
        }
    });

    if state.pending_delete == Some(board.id) {
        ui.add_space(6.0);
        confirm_delete(state, ui, palette, board, action);
    }

    if state.selected == Some(board.id) && !renaming {
        ui.add_space(6.0);
        tag_editor(state, ui, palette, board, action, typing);
    }
}

/// 块数与相对更新时间。
fn meta_text(board: &BoardItem, now: i64, palette: &theme::Palette) -> RichText {
    RichText::new(format!(
        "{} {} · {}",
        board.entry_count,
        strings::HOME_BLOCK_COUNT,
        relative_time(board.updated_at, now)
    ))
    .size(theme::SMALL_FONT)
    .color(palette.muted)
}

fn relative_time(updated_at: i64, now: i64) -> String {
    let elapsed = now.saturating_sub(updated_at).max(0);
    if elapsed < JUST_NOW_SECS {
        return strings::HOME_UPDATED_JUST_NOW.to_owned();
    }
    if elapsed < DAY_SECS {
        return strings::HOME_UPDATED_TODAY.to_owned();
    }
    if elapsed < DAY_SECS * 2 {
        return strings::HOME_UPDATED_YESTERDAY.to_owned();
    }
    let days = elapsed / DAY_SECS;
    format!("{days} {}", strings::HOME_UPDATED_DAYS)
}

fn rename_field(
    state: &mut HomeState,
    ui: &mut Ui,
    board: &BoardItem,
    action: &mut Option<HomeAction>,
    typing: &mut bool,
) {
    let mut commit: Option<String> = None;
    let mut cancel = false;

    if let Some((_, text)) = state.editing.as_mut() {
        let id = ui.make_persistent_id(("rename", board.id));
        let response = ui.add(
            TextEdit::singleline(text)
                .id(id)
                .desired_width(ui.available_width())
                .font(FontId::proportional(theme::BLOCK_FONT)),
        );
        if state.rename_focus {
            response.request_focus();
            state.rename_focus = false;
        }
        *typing |= response.has_focus();
        if ui.input(|input| input.key_pressed(Key::Escape)) {
            cancel = true;
        } else if response.lost_focus() {
            if ui.input(|input| input.key_pressed(Key::Enter)) {
                commit = Some(text.trim().to_owned());
            } else {
                cancel = true;
            }
        }
    }

    if cancel || commit.is_some() {
        state.editing = None;
    }
    if let Some(title) = commit
        && action.is_none()
    {
        *action = Some(HomeAction::Rename(board.id, title));
    }
}

fn confirm_delete(
    state: &mut HomeState,
    ui: &mut Ui,
    palette: &theme::Palette,
    board: &BoardItem,
    action: &mut Option<HomeAction>,
) {
    ui.label(
        RichText::new(strings::HOME_CONFIRM_DELETE)
            .size(theme::SMALL_FONT)
            .color(palette.muted),
    );
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.button(strings::HOME_CANCEL).clicked() {
            state.pending_delete = None;
        }
        if ui.button(strings::HOME_CONFIRM).clicked() {
            state.pending_delete = None;
            state.editing = None;
            if action.is_none() {
                *action = Some(HomeAction::Delete(board.id));
            }
        }
    });
}

/// 选中画板的标签：点标签删掉，输入框回车加一个。
fn tag_editor(
    state: &mut HomeState,
    ui: &mut Ui,
    palette: &theme::Palette,
    board: &BoardItem,
    action: &mut Option<HomeAction>,
    typing: &mut bool,
) {
    let mut remove = None;
    let mut add: Option<Vec<String>> = None;

    ui.horizontal(|ui| {
        for (index, tag) in board.tags.iter().enumerate() {
            if tag_chip(ui, palette, tag, true) {
                remove = Some(index);
            }
        }
        let response = ui.add(
            TextEdit::singleline(&mut state.tag_draft)
                .id(ui.make_persistent_id(("tag-input", board.id)))
                .hint_text(strings::HOME_ADD_TAG)
                .desired_width(120.0),
        );
        if state.tag_focus {
            response.request_focus();
            state.tag_focus = false;
        }
        *typing |= response.has_focus();
        if response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
            let draft = state.tag_draft.trim();
            if !draft.is_empty() {
                let tags = normalized_tags(
                    board
                        .tags
                        .iter()
                        .map(String::as_str)
                        .chain(std::iter::once(draft)),
                );
                if tags.len() > board.tags.len() {
                    add = Some(tags);
                }
                // 连着加下一个标签，不用再点一次输入框。
                response.request_focus();
            }
        }
    });

    if let Some(tags) = add {
        state.tag_draft.clear();
        if action.is_none() {
            *action = Some(HomeAction::SetTags(board.id, tags));
        }
    }
    if let Some(index) = remove {
        let tags: Vec<String> = board
            .tags
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != index)
            .map(|(_, tag)| tag.clone())
            .collect();
        if action.is_none() {
            *action = Some(HomeAction::SetTags(board.id, normalized_tags(tags)));
        }
    }
}

fn board_menu(
    state: &mut HomeState,
    ui: &mut Ui,
    board: &BoardItem,
    action: &mut Option<HomeAction>,
) {
    if ui.button(strings::HOME_RENAME).clicked() {
        state.editing = Some((board.id, board.title.clone()));
        state.rename_focus = true;
        state.pending_delete = None;
        ui.close();
    }
    if ui.button(strings::HOME_TAGS).clicked() {
        state.selected = Some(board.id);
        state.tag_draft.clear();
        state.tag_focus = true;
        ui.close();
    }
    ui.menu_button(strings::HOME_EXPORT, |ui| {
        if ui.button(strings::HOME_EXPORT_CONTAINER).clicked() {
            if action.is_none() {
                *action = Some(HomeAction::Export(board.id, ExportKind::Container));
            }
            ui.close();
        }
        if ui.button(strings::HOME_EXPORT_HTML).clicked() {
            if action.is_none() {
                *action = Some(HomeAction::Export(board.id, ExportKind::Html));
            }
            ui.close();
        }
    });
    if ui.button(strings::HOME_DELETE).clicked() {
        state.pending_delete = Some(board.id);
        state.editing = None;
        ui.close();
    }
}

/// 一个标签小片。removable 时可点，返回这次点没点。
fn tag_chip(ui: &mut Ui, palette: &theme::Palette, text: &str, removable: bool) -> bool {
    let font = FontId::proportional(theme::SMALL_FONT);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), palette.muted);
    let extra = if removable { CHIP_PAD + 8.0 } else { 0.0 };
    let size = vec2(galley.size().x + CHIP_PAD * 2.0 + extra, CHIP_HEIGHT);
    let sense = if removable {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    // 行悬停/选中时底色是 media_fill，所以标签片用块底色才始终看得出来。
    let fill = if removable && response.hovered() {
        palette.hover_edge
    } else {
        palette.block_fill
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(CHIP_RADIUS), fill);
    let text_pos = pos2(
        rect.left() + CHIP_PAD,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(text_pos, galley, palette.muted);
    if removable {
        ui.painter().text(
            pos2(rect.right() - CHIP_PAD, rect.center().y),
            Align2::CENTER_CENTER,
            "×",
            font,
            palette.muted,
        );
    }
    removable && response.clicked()
}

/// 匹配词高亮：命中段用正文色 + 浅底，其余用灰字。深色与浅色都只取主题色。
fn snippet_job(text: &str, needle: &str, palette: &theme::Palette) -> LayoutJob {
    let font = FontId::proportional(theme::SMALL_FONT);
    let normal = TextFormat {
        font_id: font.clone(),
        color: palette.muted,
        ..Default::default()
    };
    let hit = TextFormat {
        font_id: font,
        color: palette.text,
        // 行悬停/选中时底色是 media_fill；用 edge 当高亮底才不会被吃掉。
        background: palette.edge,
        ..Default::default()
    };

    let mut job = LayoutJob::default();
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;

    let mut cursor = 0;
    for (start, end) in highlight_ranges(text, needle) {
        if start > cursor {
            job.append(&text[cursor..start], 0.0, normal.clone());
        }
        job.append(&text[start..end], 0.0, hit.clone());
        cursor = end;
    }
    if cursor < text.len() {
        job.append(&text[cursor..], 0.0, normal);
    }
    job
}

/// 找出 needle 在 text 里所有不区分大小写的字节区间。
///
/// 只在大小写转换不改变字节长度时做（中文与 ASCII 都满足）；遇到会改变长度
/// 的文字（少数带组合符号的字母）就退回不高亮，不猜位置。
fn highlight_ranges(text: &str, needle: &str) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return Vec::new();
    }
    let lowered = text.to_lowercase();
    if lowered.len() != text.len() {
        return Vec::new();
    }
    let needle = needle.to_lowercase();
    let mut ranges = Vec::new();
    let mut from = 0;
    while let Some(offset) = lowered[from..].find(&needle) {
        let start = from + offset;
        let end = start + needle.len();
        ranges.push((start, end));
        from = end;
    }
    ranges
}

/// 去空、去重、保序。
fn normalized_tags<I, S>(tags: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut normalized: Vec<String> = Vec::new();
    for tag in tags {
        let tag = tag.as_ref().trim();
        if tag.is_empty() || normalized.iter().any(|existing| existing.as_str() == tag) {
            continue;
        }
        normalized.push(tag.to_owned());
    }
    normalized
}

fn handle_keys(
    state: &mut HomeState,
    visible: &[&BoardItem],
    ui: &Ui,
    action: &mut Option<HomeAction>,
) {
    // 新建不挑焦点，任何时候都能按。
    if ui.input(|input| input.modifiers.command && input.key_pressed(Key::N)) {
        *action = Some(HomeAction::Create);
        return;
    }
    // 在文本框里打字时，方向键是移光标，不是选行。
    if state.typing || state.editing.is_some() || visible.is_empty() {
        return;
    }
    let (up, down, enter) = ui.input(|input| {
        (
            input.key_pressed(Key::ArrowUp),
            input.key_pressed(Key::ArrowDown),
            input.key_pressed(Key::Enter),
        )
    });
    if up {
        step_selection(state, visible, -1);
    }
    if down {
        step_selection(state, visible, 1);
    }
    if enter
        && action.is_none()
        && let Some(id) = state.selected
    {
        *action = Some(HomeAction::Open(id));
    }
}

fn step_selection(state: &mut HomeState, visible: &[&BoardItem], delta: isize) {
    let current = state
        .selected
        .and_then(|id| visible.iter().position(|board| board.id == id));
    let next = match current {
        Some(index) => (index as isize + delta).clamp(0, visible.len() as isize - 1) as usize,
        None => {
            if delta > 0 {
                0
            } else {
                visible.len() - 1
            }
        }
    };
    let id = visible[next].id;
    if state.selected != Some(id) {
        state.tag_draft.clear();
    }
    state.selected = Some(id);
    state.scroll_to = Some(id);
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_time_buckets_are_stable() {
        let now = 1_700_000_000;
        assert_eq!(relative_time(now, now), strings::HOME_UPDATED_JUST_NOW);
        assert_eq!(
            relative_time(now - JUST_NOW_SECS, now),
            strings::HOME_UPDATED_TODAY
        );
        assert_eq!(
            relative_time(now - DAY_SECS, now),
            strings::HOME_UPDATED_YESTERDAY
        );
        assert_eq!(
            relative_time(now - DAY_SECS * 3, now),
            format!("3 {}", strings::HOME_UPDATED_DAYS)
        );
    }

    #[test]
    fn tags_are_trimmed_deduped_and_ordered() {
        let tags = normalized_tags(["b", " a ", "", "b", "c"]);
        assert_eq!(tags, vec!["b".to_owned(), "a".to_owned(), "c".to_owned()]);
    }

    #[test]
    fn highlight_is_case_insensitive() {
        assert_eq!(
            highlight_ranges("Hello hello", "hello"),
            vec![(0, 5), (6, 11)]
        );
        assert_eq!(highlight_ranges("没有命中", "画板"), Vec::new());
        assert_eq!(highlight_ranges("abc", ""), Vec::new());
    }

    fn board(id: u64, tags: &[&str]) -> BoardItem {
        BoardItem {
            id,
            title: String::new(),
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            snippet: None,
            updated_at: 0,
            entry_count: 0,
        }
    }

    #[test]
    fn filter_intersects_selected_tags() {
        let tags = vec!["a".to_owned(), "b".to_owned()];
        assert!(matches_filter(&tags, &[]));
        assert!(matches_filter(&tags, &["a".to_owned()]));
        assert!(matches_filter(&tags, &["a".to_owned(), "b".to_owned()]));
        assert!(!matches_filter(&tags, &["a".to_owned(), "c".to_owned()]));
    }

    #[test]
    fn all_tags_keeps_first_seen_order() {
        let boards = vec![board(1, &["b", "a"]), board(2, &["a", "c"])];
        assert_eq!(
            all_tags(&boards),
            vec!["b".to_owned(), "a".to_owned(), "c".to_owned()]
        );
    }

    #[test]
    fn cover_uv_crops_only_the_long_side() {
        let wide = cover_uv(Vec2::new(200.0, 100.0), Vec2::new(56.0, 40.0));
        assert!(wide.width() < 1.0);
        assert_eq!(wide.height(), 1.0);
        let tall = cover_uv(Vec2::new(100.0, 200.0), Vec2::new(56.0, 40.0));
        assert!(tall.height() < 1.0);
        assert_eq!(tall.width(), 1.0);
    }

    #[test]
    fn thumbnail_png_decodes_and_downscales() {
        let source = image::RgbaImage::from_pixel(320, 200, image::Rgba([9, 8, 7, 255]));
        let mut bytes = Vec::new();
        source
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("encode png");
        let (size, rgba) = rgba_thumbnail(&bytes).expect("decode png");
        assert!(size[0] <= MAX_THUMBNAIL_SIDE as usize);
        assert!(size[1] <= MAX_THUMBNAIL_SIDE as usize);
        assert_eq!(rgba.len(), size[0] * size[1] * 4);
        assert!(rgba_thumbnail(b"not a png").is_none());
    }

    struct CountingSource {
        calls: std::rc::Rc<std::cell::Cell<usize>>,
    }

    impl ThumbnailSource for CountingSource {
        fn thumbnail(&self, _board_id: u64, _updated_at: i64) -> Option<Vec<u8>> {
            self.calls.set(self.calls.get() + 1);
            let source = image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]));
            let mut bytes = Vec::new();
            source
                .write_to(
                    &mut std::io::Cursor::new(&mut bytes),
                    image::ImageFormat::Png,
                )
                .ok()?;
            Some(bytes)
        }
    }

    #[test]
    fn home_frame_paints_thumbnails_once_per_board() {
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let mut app = HomeApp::with_thumbnails(Box::new(CountingSource {
            calls: calls.clone(),
        }));
        app.set_boards(vec![
            BoardItem {
                id: 1,
                title: "board".to_owned(),
                tags: vec!["t".to_owned()],
                snippet: None,
                updated_at: 10,
                entry_count: 2,
            },
            board(2, &["t"]),
        ]);

        let ctx = egui::Context::default();
        let frame = |app: &mut HomeApp| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1280.0, 760.0),
                )),
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                assert!(app.ui(ui).is_none());
            });
            // 测试里没有渲染后端消费纹理上传，丢掉增量。
            output.textures_delta.clear();
        };

        frame(&mut app);
        // 第一帧两张画板各取一次。
        assert_eq!(calls.get(), 2);
        frame(&mut app);
        // 第二帧命中 (id, updated_at) 缓存，不再向来源取字节。
        assert_eq!(calls.get(), 2);
    }
}
