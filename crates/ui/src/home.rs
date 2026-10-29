#![forbid(unsafe_code)]

//! 左侧边栏：笔记列表。
//!
//! 只画列表本身：顶部一行放「新建」，下面按更新时间倒序的行，提示压在面板底部。
//! 界面只产出一个动作，读写由 app 做。时刻由 app 注入（`set_now`），界面不读系统时钟：
//! 分组与具体日期都要本地时区，本轮不加日期库，所以行内只写相对时间。

use std::cmp::Reverse;
use std::sync::Arc;
use std::time::Duration;

use egui::text::{Galley, LayoutJob, TextWrapping};
use egui::{
    Align, Color32, CornerRadius, FontId, Key, Layout, Rect, Sense, Stroke, TextEdit, Ui, Vec2,
    pos2, vec2,
};

use crate::icons::{self, Icon};
use crate::shell::TOP_BAR_HEIGHT;
use crate::strings;
use crate::theme;

/// 一行笔记的高度：标题一行、时间一行。
const ROW_HEIGHT: f32 = 52.0;
/// 图标在 28 的按钮里占 16，四边各留 6。
const ICON_INSET: f32 = 6.0;
/// 空态从列表顶部往下放的比例。
const EMPTY_ANCHOR: f32 = 0.3;
/// 成功提示停留的秒数；失败提示常驻，等用户处理。
const STATUS_LIFETIME: f64 = 2.4;
const MINUTE: i64 = 60;
const HOUR: i64 = 3_600;
const DAY: i64 = 86_400;

/// 列表里的一条笔记。
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

/// 按需取缩略图的来源，返回 PNG 字节。
///
/// 本轮列表只画标题与时间，还不用它；签名先留着，下一轮资源栏要接。
pub trait ThumbnailSource {
    fn thumbnail(&self, board_id: u64, updated_at: i64) -> Option<Vec<u8>>;
}

/// 不需要缩略图时的默认来源。
impl ThumbnailSource for () {
    fn thumbnail(&self, _board_id: u64, _updated_at: i64) -> Option<Vec<u8>> {
        None
    }
}

/// 侧栏列表状态：列表、正在编辑的行、底部提示。
pub struct HomeApp {
    boards: Vec<BoardItem>,
    state: HomeState,
    /// app 注入的当前时刻（Unix 秒），相对时间只用它算。
    now: i64,
    /// 当前打开的那条笔记，用来决定选中底。
    current: Option<u64>,
    /// 缩略图来源：本轮不画缩略图，接口先留着。
    _thumbnails: Box<dyn ThumbnailSource>,
}

/// 跟列表内容无关的交互状态。
#[derive(Default)]
struct HomeState {
    /// 正在重命名的那条笔记与输入框里的草稿。
    editing: Option<(u64, String)>,
    /// 刚进入重命名时把焦点给输入框。
    rename_focus: bool,
    /// 正在改标签的那条笔记与输入框里的草稿。
    tags: Option<(u64, String)>,
    /// 标签输入框刚打开时把焦点给它。
    tags_focus: bool,
    /// 这一帧有输入框拿着焦点，方向键留给光标。
    typing: bool,
    status: Option<Status>,
    /// 键盘光标：高亮哪一行。打开别的笔记时跟过去。
    selected: Option<u64>,
    /// 键盘换了选中行，下一帧把它滚进视野。
    scroll_to: Option<u64>,
    /// 上一帧列表可视区 (滚动偏移, 高度)。
    scroll_viewport: (f32, f32),
    /// 下一帧强制滚动到的偏移（像素）。
    scroll_override: Option<f32>,
    /// 上一帧实际布局了多少行；测试用它确认虚拟滚动没有偷偷排全表。
    rendered_rows: usize,
}

/// 面板底部的一条提示。
struct Status {
    text: String,
    /// 失败要用户看见，所以常驻并用 danger 色。
    is_error: bool,
    /// 第一次画出来的帧时刻；成功提示从这里开始计时。
    since: Option<f64>,
}

impl HomeState {
    /// 成功提示到点就撤；失败提示留着。
    fn tick_status(&mut self, time: f64) {
        let Some(status) = self.status.as_mut() else {
            return;
        };
        let since = *status.since.get_or_insert(time);
        let expired = !status.is_error && time - since >= STATUS_LIFETIME;
        if expired {
            self.status = None;
        }
    }
}

impl HomeApp {
    pub fn new() -> Self {
        Self::with_thumbnails(Box::new(()))
    }

    /// 用给定的缩略图来源建列表；new 用不提供缩略图的默认来源。
    pub fn with_thumbnails(source: Box<dyn ThumbnailSource>) -> Self {
        Self {
            boards: Vec::new(),
            state: HomeState::default(),
            now: 0,
            current: None,
            _thumbnails: source,
        }
    }

    /// 回填列表。按更新时间倒序排；同一秒的保持 app 给的顺序（稳定排序）。
    pub fn set_boards(&mut self, boards: Vec<BoardItem>) {
        let mut boards = boards;
        boards.sort_by_key(|board| Reverse(board.updated_at));
        self.boards = boards;
        let ids: Vec<u64> = self.boards.iter().map(|board| board.id).collect();
        let state = &mut self.state;
        if state.selected.is_some_and(|id| !ids.contains(&id)) {
            state.selected = None;
        }
        if state
            .editing
            .as_ref()
            .is_some_and(|(id, _)| !ids.contains(id))
        {
            state.editing = None;
        }
        if state.tags.as_ref().is_some_and(|(id, _)| !ids.contains(id)) {
            state.tags = None;
        }
    }

    /// app 每帧注入当前时刻（Unix 秒）。界面不读系统时钟。
    pub fn set_now(&mut self, unix_seconds: i64) {
        self.now = unix_seconds;
    }

    /// 当前打开的笔记，用来画选中底；换了笔记时键盘光标跟过去。
    pub fn set_current(&mut self, id: Option<u64>) {
        if self.current != id {
            self.current = id;
            self.state.selected = id;
        }
    }

    /// 一条会自己消失的成功提示。
    pub fn set_status(&mut self, status: impl Into<String>) {
        self.state.status = Some(Status {
            text: status.into(),
            is_error: false,
            since: None,
        });
    }

    /// 一条失败提示：用 danger 色且常驻，等用户处理后由下一次操作覆盖。
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.state.status = Some(Status {
            text: message.into(),
            is_error: true,
            since: None,
        });
    }

    /// 画一帧侧栏列表，返回用户这一帧想做的动作。
    pub fn sidebar(&mut self, ui: &mut egui::Ui, query: &mut String) -> Option<HomeAction> {
        let mut action = None;
        let Self {
            boards, state, now, ..
        } = self;
        let now = *now;
        let time = ui.input(|input| input.time);
        state.tick_status(time);
        state.typing = false;

        head_row(ui, query, &mut action);

        let footer = if state.status.is_some() {
            theme::ui().control_height
        } else {
            0.0
        };
        let list_size = vec2(
            ui.available_width(),
            (ui.available_height() - footer).max(0.0),
        );
        ui.allocate_ui_with_layout(list_size, Layout::top_down(Align::Min), |ui| {
            if boards.is_empty() {
                let blank = ui.max_rect();
                empty_state(ui);
                blank_menu(ui, blank, &mut action);
            } else {
                // 行与行之间不留间距：悬停底要连成一条。
                ui.spacing_mut().item_spacing.y = 0.0;
                sync_scroll(state, boards);
                let mut area = egui::ScrollArea::vertical().auto_shrink([false, false]);
                if let Some(offset) = state.scroll_override.take() {
                    area = area.vertical_scroll_offset(offset);
                }
                let output = area.show_rows(ui, ROW_HEIGHT, boards.len(), |ui, rows| {
                    let rendered = rows.len();
                    for index in rows {
                        board_row(state, ui, &boards[index], now, &mut action);
                    }
                    rendered
                });
                state.rendered_rows = output.inner;
                state.scroll_viewport =
                    (output.state.clamped_offset().y, output.inner_rect.height());
                // 最后一行下面那块空地方也能右键新建；行占满可视区时这块是零高。
                let blank = blank_below(
                    output.inner_rect,
                    output.content_size,
                    output.state.clamped_offset().y,
                );
                blank_menu(ui, blank, &mut action);
            }
            // 空列表时也要能按快捷键新建。
            handle_keys(state, boards, ui, &mut action);
        });

        status_row(state, ui, time);
        action
    }
}

impl Default for HomeApp {
    fn default() -> Self {
        Self::new()
    }
}

/// 顶部一行：左边搜索框，右边一个「＋」。不写标题，也不写计数。
///
/// 搜索贴着列表放：它是筛这张列表的，不是全局的，放顶栏会让人以为是全站搜索。
fn head_row(ui: &mut Ui, query: &mut String, action: &mut Option<HomeAction>) {
    let (_, row) = ui.allocate_space(vec2(ui.available_width(), TOP_BAR_HEIGHT));
    let control = theme::ui().control_height;
    let inner = row.shrink2(Vec2::new(
        // 与下面行的文字同一条左边线：搜索框和内缩对不齐最扎眼。
        theme::ui().gap_l,
        (TOP_BAR_HEIGHT - control) / 2.0,
    ));
    let plus = Rect::from_center_size(
        pos2(inner.right() - control / 2.0, inner.center().y),
        Vec2::splat(control),
    );
    let field = Rect::from_min_max(inner.min, pos2(plus.min.x - theme::ui().gap_m, inner.max.y));
    ui.painter().rect_filled(
        field,
        CornerRadius::same(theme::ui().radius_control),
        theme::chrome().field,
    );
    icons::draw(
        ui.painter(),
        Rect::from_center_size(
            pos2(field.min.x + 14.0, field.center().y),
            Vec2::splat(14.0),
        ),
        Icon::Search,
        theme::chrome().text_faint,
    );
    let text_rect = Rect::from_min_max(
        pos2(field.min.x + 26.0, field.min.y + 4.0),
        pos2(field.max.x - theme::ui().gap_s, field.max.y - 4.0),
    );
    let id = ui.scope_id().with("note-search");
    let before = query.clone();
    let edit = egui::TextEdit::singleline(query)
        .id(id)
        .frame(egui::Frame::NONE)
        .hint_text(strings::SHELL_SEARCH)
        .desired_width(text_rect.width());
    ui.put(text_rect, edit);
    if ui
        .ctx()
        .input(|input| input.modifiers.command && input.key_pressed(egui::Key::K))
    {
        ui.memory_mut(|memory| memory.request_focus(id));
    }
    if *query != before {
        *action = Some(HomeAction::Search(query.clone()));
    }
    if icon_button(ui, plus, Icon::Plus, strings::SIDEBAR_NEW) {
        *action = Some(HomeAction::Create);
    }
}

/// 一个安静的图标按钮：悬停才出底，图标跟着变深。
fn icon_button(ui: &mut Ui, rect: Rect, icon: Icon, label: &str) -> bool {
    let response = ui.interact(rect, ui.scope_id().with(label), Sense::click());
    let hovered = response.hovered();
    if hovered {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(theme::ui().radius_control),
            theme::chrome().hover,
        );
    }
    let color = if hovered {
        theme::chrome().text
    } else {
        theme::chrome().text_muted
    };
    icons::draw(ui.painter(), rect.shrink(ICON_INSET), icon, color);
    response.on_hover_text(label).clicked()
}

/// 一条笔记的行：悬停/选中只换底色，几何不动。
fn board_row(
    state: &mut HomeState,
    ui: &mut Ui,
    board: &BoardItem,
    now: i64,
    action: &mut Option<HomeAction>,
) {
    let selected = state.selected == Some(board.id);
    let renaming = state
        .editing
        .as_ref()
        .is_some_and(|(id, _)| *id == board.id);
    let tagging = state.tags.as_ref().is_some_and(|(id, _)| *id == board.id);

    let (_, row) = ui.allocate_space(vec2(ui.available_width(), ROW_HEIGHT));
    let response = ui.interact(
        row,
        ui.scope_id().with(("home-row", board.id)),
        Sense::click(),
    );
    let bg = Rect::from_min_max(
        pos2(row.left() + theme::ui().gap_m, row.top()),
        pos2(row.right() - theme::ui().gap_m, row.bottom()),
    );
    let more_rect = Rect::from_center_size(
        pos2(
            bg.right() - theme::ui().gap_m - theme::ui().control_height / 2.0,
            row.center().y,
        ),
        Vec2::splat(theme::ui().control_height),
    );
    let more = ui.interact(
        more_rect,
        ui.scope_id().with(("home-more", board.id)),
        Sense::click(),
    );
    // 右键落在「更多」上也按行右键处理。union 的 id 取自行，和「更多」自己那个浮层不冲突。
    let row_click = response.union(more.clone());

    // 菜单开着时指针在菜单上，这一行仍算悬停：底色和「更多」不会先消失。
    let popup = egui::Popup::from_toggle_button_response(&more)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(menu_frame(ui));
    // 右键菜单和「更多」一样用阴影浮层，不加边框、不加动效。
    let context = egui::Popup::context_menu(&row_click)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(menu_frame(ui));
    let hovered = response.contains_pointer()
        || more.contains_pointer()
        || popup.is_open()
        || context.is_open();
    let fill = if selected {
        Some(theme::chrome().selected)
    } else if hovered {
        Some(theme::chrome().hover)
    } else {
        None
    };
    if let Some(fill) = fill {
        ui.painter()
            .rect_filled(bg, CornerRadius::same(theme::ui().radius_control), fill);
    }
    // 「更多」只在悬停或选中时出现；打开菜单后指针离开行也还在。
    if selected || hovered {
        let color = if more.contains_pointer() {
            theme::chrome().text
        } else {
            theme::chrome().text_muted
        };
        icons::draw(
            ui.painter(),
            more_rect.shrink(ICON_INSET),
            Icon::More,
            color,
        );
    }
    popup.show(|ui| board_menu(state, ui, board, action));
    context.show(|ui| row_context_menu(state, ui, board, action));

    let content = Rect::from_min_max(
        pos2(bg.left() + theme::ui().gap_m, row.top()),
        pos2(more_rect.left() - theme::ui().gap_s, row.bottom()),
    );
    let typing = row_content(state, ui, content, board, now, action);
    state.typing |= typing;

    if response.clicked() && !more.clicked() && !renaming && !tagging && action.is_none() {
        state.selected = Some(board.id);
        *action = Some(HomeAction::Open(board.id));
    }
}

/// 行里的两行字：标题一行、时间一行；重命名与改标签时换成输入框。
fn row_content(
    state: &mut HomeState,
    ui: &mut Ui,
    content: Rect,
    board: &BoardItem,
    now: i64,
    action: &mut Option<HomeAction>,
) -> bool {
    let renaming = state
        .editing
        .as_ref()
        .is_some_and(|(id, _)| *id == board.id);
    let tagging = state.tags.as_ref().is_some_and(|(id, _)| *id == board.id);
    let left = content.left();
    let width = content.width();
    let center = content.center().y;
    let gap = theme::ui().gap_s;
    let field_height = theme::ui().control_height;

    let title = if board.title.trim().is_empty() {
        strings::HOME_UNTITLED
    } else {
        board.title.as_str()
    };
    let title_galley = line_galley(
        ui,
        title,
        FontId::proportional(theme::ui().body),
        theme::chrome().text,
        width,
    );
    let meta_galley = line_galley(
        ui,
        &relative_time(board.updated_at, now),
        FontId::proportional(theme::ui().meta),
        theme::chrome().text_faint,
        width,
    );
    let title_height = title_galley.size().y;
    let meta_height = meta_galley.size().y;
    let mut typing = false;

    if renaming {
        let top = center - (field_height + gap + meta_height) / 2.0;
        let field = Rect::from_min_size(pos2(left, top), vec2(width, field_height));
        typing |= rename_field(state, ui, board, field, action);
        ui.painter().galley(
            pos2(left, top + field_height + gap),
            meta_galley,
            theme::chrome().text,
        );
    } else if tagging {
        let top = center - (title_height + gap + field_height) / 2.0;
        ui.painter()
            .galley(pos2(left, top), title_galley, theme::chrome().text);
        let field = Rect::from_min_size(
            pos2(left, top + title_height + gap),
            vec2(width, field_height),
        );
        typing |= tags_field(state, ui, board, field, action);
    } else {
        let top = center - (title_height + gap + meta_height) / 2.0;
        ui.painter()
            .galley(pos2(left, top), title_galley, theme::chrome().text);
        ui.painter().galley(
            pos2(left, top + title_height + gap),
            meta_galley,
            theme::chrome().text,
        );
    }
    typing
}

/// 一行不换行、放不下就截断加省略号的文字。
fn line_galley(ui: &Ui, text: &str, font: FontId, color: Color32, width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = TextWrapping::truncate_at_width(width);
    ui.painter().layout_job(job)
}

/// 重命名输入框。Enter 提交、Escape 取消、点走当取消。
fn rename_field(
    state: &mut HomeState,
    ui: &mut Ui,
    board: &BoardItem,
    rect: Rect,
    action: &mut Option<HomeAction>,
) -> bool {
    let mut typing = false;
    let mut commit: Option<String> = None;
    let mut cancel = false;
    if let Some((_, text)) = state.editing.as_mut() {
        let response = ui.put(
            rect,
            TextEdit::singleline(text)
                .id(ui.scope_id().with(("home-rename", board.id)))
                .desired_width(rect.width())
                .font(FontId::proportional(theme::ui().body)),
        );
        if state.rename_focus {
            response.request_focus();
            state.rename_focus = false;
        }
        typing |= response.has_focus();
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
    typing
}

/// 标签输入框：逗号分开的标签，Enter 提交。
fn tags_field(
    state: &mut HomeState,
    ui: &mut Ui,
    board: &BoardItem,
    rect: Rect,
    action: &mut Option<HomeAction>,
) -> bool {
    let mut typing = false;
    let mut commit = false;
    let mut cancel = false;
    if let Some((_, text)) = state.tags.as_mut() {
        let response = ui.put(
            rect,
            TextEdit::singleline(text)
                .id(ui.scope_id().with(("home-tags", board.id)))
                .hint_text(strings::SIDEBAR_TAGS_HINT)
                .desired_width(rect.width())
                .font(FontId::proportional(theme::ui().body)),
        );
        if state.tags_focus {
            response.request_focus();
            state.tags_focus = false;
        }
        typing |= response.has_focus();
        if ui.input(|input| input.key_pressed(Key::Escape)) {
            cancel = true;
        } else if response.lost_focus() {
            if ui.input(|input| input.key_pressed(Key::Enter)) {
                commit = true;
            } else {
                cancel = true;
            }
        }
    }
    if cancel {
        state.tags = None;
    }
    if commit
        && let Some((_, text)) = state.tags.take()
        && action.is_none()
    {
        let tags = split_tags(&text);
        if tags != board.tags {
            *action = Some(HomeAction::SetTags(board.id, tags));
        }
    }
    typing
}

/// 一条笔记的「更多」菜单。
fn board_menu(
    state: &mut HomeState,
    ui: &mut Ui,
    board: &BoardItem,
    action: &mut Option<HomeAction>,
) {
    if ui.button(strings::HOME_RENAME).clicked() {
        state.editing = Some((board.id, board.title.clone()));
        state.rename_focus = true;
        state.tags = None;
        ui.close();
    }
    if ui.button(strings::HOME_TAGS).clicked() {
        state.selected = Some(board.id);
        state.tags = Some((board.id, joined_tags(&board.tags)));
        state.tags_focus = true;
        state.editing = None;
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
        state.editing = None;
        state.tags = None;
        if action.is_none() {
            *action = Some(HomeAction::Delete(board.id));
        }
        ui.close();
    }
}

/// 笔记行的右键菜单：比「更多」多一个「打开」，其余动作走同一条路。
fn row_context_menu(
    state: &mut HomeState,
    ui: &mut Ui,
    board: &BoardItem,
    action: &mut Option<HomeAction>,
) {
    if ui.button(strings::HOME_OPEN).clicked() {
        state.selected = Some(board.id);
        if action.is_none() {
            *action = Some(HomeAction::Open(board.id));
        }
        ui.close();
    }
    board_menu(state, ui, board, action);
}

/// 列表空白处的右键菜单：只放「新建笔记」。
fn blank_menu(ui: &mut Ui, rect: Rect, action: &mut Option<HomeAction>) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let response = ui.interact(rect, ui.scope_id().with("home-blank"), Sense::click());
    egui::Popup::context_menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(menu_frame(ui))
        .show(|ui| {
            if ui.button(strings::SIDEBAR_NEW).clicked() {
                if action.is_none() {
                    *action = Some(HomeAction::Create);
                }
                ui.close();
            }
        });
}

/// 列表最后一行下面的空白区；行占满可视区时返回零高矩形。
fn blank_below(inner_rect: Rect, content_size: Vec2, offset: f32) -> Rect {
    let bottom = inner_rect.top() + content_size.y - offset;
    let top = bottom.clamp(inner_rect.top(), inner_rect.bottom());
    Rect::from_min_max(pos2(inner_rect.left(), top), inner_rect.max)
}

/// 弹出菜单的底：只有阴影和圆角，不加边框。
fn menu_frame(ui: &Ui) -> egui::Frame {
    egui::Frame::menu(ui.style())
        .corner_radius(CornerRadius::same(theme::ui().radius_surface))
        .stroke(Stroke::NONE)
}

/// 一条笔记都没有时：说清状态，并指出下一步。
fn empty_state(ui: &mut Ui) {
    ui.add_space(ui.available_height() * EMPTY_ANCHOR);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(strings::HOME_EMPTY)
                .size(theme::ui().body)
                .color(theme::chrome().text_muted),
        );
    });
}

/// 面板底部的一行提示：成功会自己消失，失败常驻并用 danger 色。
fn status_row(state: &HomeState, ui: &mut Ui, time: f64) {
    let Some(status) = state.status.as_ref() else {
        return;
    };
    if !status.is_error {
        let since = status.since.unwrap_or(time);
        let left = (STATUS_LIFETIME - (time - since)).max(0.0);
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f64(left));
    }
    let (_, rect) = ui.allocate_space(vec2(ui.available_width(), theme::ui().control_height));
    let color = if status.is_error {
        theme::chrome().danger
    } else {
        theme::chrome().text_muted
    };
    let width = rect.width() - theme::ui().gap_m * 2.0;
    let galley = line_galley(
        ui,
        &status.text,
        FontId::proportional(theme::ui().body),
        color,
        width,
    );
    let position = pos2(
        rect.left() + theme::ui().gap_m,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(position, galley, color);
}

/// 行里的时间：只写相对时刻，不写具体日期。
fn relative_time(updated_at: i64, now: i64) -> String {
    let elapsed = now.saturating_sub(updated_at).max(0);
    if elapsed < MINUTE {
        strings::SIDEBAR_TIME_JUST_NOW.to_owned()
    } else if elapsed < HOUR {
        format!("{} {}", elapsed / MINUTE, strings::SIDEBAR_TIME_MINUTES)
    } else if elapsed < DAY {
        format!("{} {}", elapsed / HOUR, strings::SIDEBAR_TIME_HOURS)
    } else if elapsed < DAY * 2 {
        strings::SIDEBAR_TIME_YESTERDAY.to_owned()
    } else {
        format!("{} {}", elapsed / DAY, strings::SIDEBAR_TIME_DAYS)
    }
}

/// 把标签写进输入框：用界面上那个逗号分开。
fn joined_tags(tags: &[String]) -> String {
    let mut text = String::new();
    for (index, tag) in tags.iter().enumerate() {
        if index > 0 {
            text.push(strings::TAG_SEPARATOR);
        }
        text.push_str(tag);
    }
    text
}

/// 输入框里逗号分开的标签：去空、去重、保序。
fn split_tags(text: &str) -> Vec<String> {
    normalized_tags(text.split([',', strings::TAG_SEPARATOR]))
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

/// 键盘光标不在可视区时，下一帧直接给滚动区一个目标偏移。
fn sync_scroll(state: &mut HomeState, boards: &[BoardItem]) {
    let Some(target) = state.scroll_to.take() else {
        return;
    };
    let Some(index) = boards.iter().position(|board| board.id == target) else {
        return;
    };
    let (offset, height) = state.scroll_viewport;
    if height <= 0.0 {
        // 还没量到可视高度（第一帧）：可见行本来就在屏幕里。
        return;
    }
    let top = index as f32 * ROW_HEIGHT;
    let bottom = top + ROW_HEIGHT;
    if top < offset {
        state.scroll_override = Some(top);
    } else if bottom > offset + height {
        state.scroll_override = Some((bottom - height).max(0.0));
    }
}

/// 方向键挪光标、Enter 打开。新建键归 app 管，这里不重复处理。
fn handle_keys(
    state: &mut HomeState,
    boards: &[BoardItem],
    ui: &Ui,
    action: &mut Option<HomeAction>,
) {
    // 在输入框里打字时，方向键是移光标，不是选行。
    if state.typing || state.editing.is_some() || state.tags.is_some() || boards.is_empty() {
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
        step_selection(state, boards, -1);
    }
    if down {
        step_selection(state, boards, 1);
    }
    if enter
        && action.is_none()
        && let Some(id) = state.selected
    {
        *action = Some(HomeAction::Open(id));
    }
}

fn step_selection(state: &mut HomeState, boards: &[BoardItem], delta: isize) {
    let current = state
        .selected
        .and_then(|id| boards.iter().position(|board| board.id == id));
    let next = match current {
        Some(index) => (index as isize + delta).clamp(0, boards.len() as isize - 1) as usize,
        None => {
            if delta > 0 {
                0
            } else {
                boards.len() - 1
            }
        }
    };
    let id = boards[next].id;
    state.selected = Some(id);
    state.scroll_to = Some(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board(id: u64, updated_at: i64) -> BoardItem {
        BoardItem {
            id,
            title: format!("note {id}"),
            tags: Vec::new(),
            snippet: None,
            updated_at,
            entry_count: 0,
        }
    }

    fn frame_output(
        ctx: &egui::Context,
        app: &mut HomeApp,
        time: f64,
        events: Vec<egui::Event>,
    ) -> (Option<HomeAction>, egui::FullOutput) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1280.0, 760.0),
            )),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut action = None;
        let mut output = ctx.run_ui(input, |ui| {
            action = app.sidebar(ui, &mut String::new());
        });
        // 测试里没有渲染后端消费纹理上传，丢掉增量。
        output.textures_delta.clear();
        (action, output)
    }

    fn frame_events(
        ctx: &egui::Context,
        app: &mut HomeApp,
        time: f64,
        events: Vec<egui::Event>,
    ) -> Option<HomeAction> {
        frame_output(ctx, app, time, events).0
    }

    fn frame(ctx: &egui::Context, app: &mut HomeApp, time: f64) -> Option<HomeAction> {
        frame_events(ctx, app, time, Vec::new())
    }

    fn pointer_button(pos: egui::Pos2, button: egui::PointerButton, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }

    /// 一次完整的右键：移动、按下、松开。菜单在松开那一帧打开。
    fn secondary_click(
        ctx: &egui::Context,
        app: &mut HomeApp,
        time: f64,
        at: egui::Pos2,
    ) -> (Option<HomeAction>, egui::FullOutput) {
        frame_events(ctx, app, time, vec![egui::Event::PointerMoved(at)]);
        frame_events(
            ctx,
            app,
            time + 0.1,
            vec![pointer_button(at, egui::PointerButton::Secondary, true)],
        );
        frame_output(
            ctx,
            app,
            time + 0.2,
            vec![pointer_button(at, egui::PointerButton::Secondary, false)],
        )
    }

    /// 一次完整的左键：移动、按下、松开。
    fn primary_click(
        ctx: &egui::Context,
        app: &mut HomeApp,
        time: f64,
        at: egui::Pos2,
    ) -> Option<HomeAction> {
        frame_events(ctx, app, time, vec![egui::Event::PointerMoved(at)]);
        frame_events(
            ctx,
            app,
            time + 0.1,
            vec![pointer_button(at, egui::PointerButton::Primary, true)],
        );
        frame_events(
            ctx,
            app,
            time + 0.2,
            vec![pointer_button(at, egui::PointerButton::Primary, false)],
        )
    }

    /// 某一帧里某段文字的中心：菜单项的命中区就围着它。
    fn painted_label_center(output: &egui::FullOutput, text: &str) -> Option<egui::Pos2> {
        output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(shape) if shape.galley.text() == text => {
                    Some(Rect::from_min_size(shape.pos, shape.galley.size()).center())
                }
                _ => None,
            })
    }

    fn first_row_center() -> egui::Pos2 {
        pos2(100.0, TOP_BAR_HEIGHT + ROW_HEIGHT / 2.0)
    }

    /// 在第一条笔记上右键，返回菜单画稳之后的那一帧。
    fn row_context_menu_frame(ctx: &egui::Context, app: &mut HomeApp) -> egui::FullOutput {
        let (action, _) = secondary_click(ctx, app, 0.0, first_row_center());
        assert_eq!(action, None, "右键不该把笔记打开");
        // 浮层第一帧还在量尺寸，再画一帧拿稳定位置。
        frame_output(ctx, app, 0.5, Vec::new()).1
    }

    #[test]
    fn relative_time_uses_relative_buckets_only() {
        let now = 1_700_000_000;
        assert_eq!(relative_time(now, now), strings::SIDEBAR_TIME_JUST_NOW);
        assert_eq!(
            relative_time(now - 5 * MINUTE, now),
            format!("5 {}", strings::SIDEBAR_TIME_MINUTES)
        );
        assert_eq!(
            relative_time(now - 3 * HOUR, now),
            format!("3 {}", strings::SIDEBAR_TIME_HOURS)
        );
        assert_eq!(
            relative_time(now - DAY, now),
            strings::SIDEBAR_TIME_YESTERDAY
        );
        assert_eq!(
            relative_time(now - 4 * DAY, now),
            format!("4 {}", strings::SIDEBAR_TIME_DAYS)
        );
    }

    #[test]
    fn boards_are_newest_first() {
        let mut app = HomeApp::new();
        app.set_boards(vec![board(1, 10), board(2, 30), board(3, 20)]);
        let ids: Vec<u64> = app.boards.iter().map(|item| item.id).collect();
        assert_eq!(ids, vec![2, 3, 1]);
    }

    #[test]
    fn opening_a_note_selects_its_row() {
        let mut app = HomeApp::new();
        app.set_boards(vec![board(1, 10), board(2, 20)]);
        app.set_current(Some(2));
        assert_eq!(app.state.selected, Some(2));
        app.set_current(None);
        assert_eq!(app.state.selected, None);
    }

    #[test]
    fn tag_input_splits_trims_and_dedups() {
        assert_eq!(split_tags(" b, a，b,, c "), vec!["b", "a", "c"]);
        assert_eq!(joined_tags(&split_tags("a，b")), "a，b");
    }

    #[test]
    fn an_empty_list_has_no_action() {
        let mut app = HomeApp::new();
        let ctx = egui::Context::default();
        assert!(frame(&ctx, &mut app, 0.0).is_none());
    }

    #[test]
    fn a_success_status_goes_away_and_a_failure_stays() {
        let mut app = HomeApp::new();
        let ctx = egui::Context::default();
        app.set_status(strings::EXPORTED);
        frame(&ctx, &mut app, 0.0);
        assert!(app.state.status.is_some());
        frame(&ctx, &mut app, 1.0);
        assert!(app.state.status.is_some());
        frame(&ctx, &mut app, 2.5);
        assert!(app.state.status.is_none(), "成功提示应当到点消失");

        app.set_error(strings::export_failed("磁盘满了"));
        frame(&ctx, &mut app, 3.0);
        frame(&ctx, &mut app, 400.0);
        assert!(
            app.state
                .status
                .as_ref()
                .is_some_and(|status| status.is_error),
            "失败提示不许自己消失"
        );
    }

    #[test]
    fn a_thousand_rows_only_layout_visible_rows() {
        let mut app = HomeApp::new();
        app.set_boards((0..1000).map(|id| board(id, id as i64)).collect());
        app.set_now(10_000);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, 0.0);
        assert!(
            app.state.rendered_rows > 0 && app.state.rendered_rows < 40,
            "1000 行里只该布局可见行，实际布局了 {} 行",
            app.state.rendered_rows
        );
    }

    #[test]
    fn keyboard_selection_scrolls_the_list() {
        let mut app = HomeApp::new();
        app.set_boards((0..200).map(|id| board(id, 200 - id as i64)).collect());
        app.set_now(10_000);
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, 0.0);
        for step in 1..=60 {
            let event = egui::Event::Key {
                key: Key::ArrowDown,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::default(),
            };
            frame_events(&ctx, &mut app, step as f64 * 0.1, vec![event]);
        }
        let newest_first: Vec<u64> = app.boards.iter().map(|item| item.id).collect();
        assert_eq!(app.state.selected, Some(newest_first[59]));
        assert!(
            app.state.scroll_viewport.0 > 0.0,
            "选中行移出视野后列表应该跟着滚动"
        );
    }

    #[test]
    fn clicking_a_row_opens_the_note() {
        let mut app = HomeApp::new();
        app.set_boards(vec![board(7, 10)]);
        app.set_now(20);
        let ctx = egui::Context::default();
        let at = pos2(100.0, TOP_BAR_HEIGHT + ROW_HEIGHT / 2.0);
        frame_events(&ctx, &mut app, 0.0, vec![egui::Event::PointerMoved(at)]);
        frame_events(
            &ctx,
            &mut app,
            0.1,
            vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            }],
        );
        let action = frame_events(
            &ctx,
            &mut app,
            0.2,
            vec![egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            }],
        );
        assert_eq!(action, Some(HomeAction::Open(7)));
    }

    #[test]
    fn the_open_note_row_is_painted_selected() {
        let mut app = HomeApp::new();
        app.set_boards(vec![board(1, 10), board(2, 20)]);
        app.set_current(Some(2));
        app.set_now(30);
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1280.0, 760.0),
            )),
            time: Some(0.0),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            let _ = app.sidebar(ui, &mut String::new());
        });
        output.textures_delta.clear();
        assert_eq!(app.state.selected, Some(2));
        let painted = output
            .shapes
            .iter()
            .any(|clipped| matches!(&clipped.shape, egui::Shape::Rect(rect) if rect.fill == theme::chrome().selected));
        assert!(painted, "当前笔记那一行应当画上选中底");
    }

    #[test]
    fn ctrl_n_is_not_the_sidebar_business() {
        let mut app = HomeApp::new();
        let ctx = egui::Context::default();
        let modifiers = egui::Modifiers {
            command: true,
            ..Default::default()
        };
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1280.0, 760.0),
            )),
            time: Some(0.0),
            events: vec![
                egui::Event::ModifiersChanged(modifiers),
                egui::Event::Key {
                    key: Key::N,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers,
                },
            ],
            ..Default::default()
        };
        let mut action = None;
        let mut output = ctx.run_ui(input, |ui| {
            action = app.sidebar(ui, &mut String::new());
        });
        output.textures_delta.clear();
        // 新建键只在 app 里处理一次；侧栏再发一次就会一按建两张。
        assert_eq!(action, None);
    }

    #[test]
    fn secondary_click_on_a_row_opens_the_context_menu() {
        let mut app = HomeApp::new();
        app.set_boards(vec![board(7, 10)]);
        app.set_now(20);
        let ctx = egui::Context::default();
        let output = row_context_menu_frame(&ctx, &mut app);
        for label in [
            strings::HOME_OPEN,
            strings::HOME_RENAME,
            strings::HOME_TAGS,
            strings::HOME_EXPORT,
            strings::HOME_DELETE,
        ] {
            assert!(
                painted_label_center(&output, label).is_some(),
                "右键菜单里应该有「{label}」"
            );
        }
        assert_eq!(app.state.selected, None, "右键只是弹菜单，不该顺手打开笔记");
    }

    #[test]
    fn secondary_click_over_the_more_button_opens_the_same_row_menu() {
        let mut app = HomeApp::new();
        app.set_boards(vec![board(7, 10)]);
        app.set_now(20);
        let ctx = egui::Context::default();
        // 交互用的是上一帧的控件矩形：先布一帧，再让指针停在行上。
        frame_output(&ctx, &mut app, 0.0, Vec::new());
        let (_, output) = frame_output(
            &ctx,
            &mut app,
            0.1,
            vec![egui::Event::PointerMoved(first_row_center())],
        );
        let row_bg = output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(rect)
                    if rect.fill == theme::chrome().hover
                        && (rect.rect.height() - ROW_HEIGHT).abs() < 0.5 =>
                {
                    Some(rect.rect)
                }
                _ => None,
            })
            .expect("行悬停底应该画出来");
        let at = pos2(
            row_bg.right() - theme::ui().gap_m - theme::ui().control_height / 2.0,
            row_bg.center().y,
        );
        secondary_click(&ctx, &mut app, 0.2, at);
        let output = frame_output(&ctx, &mut app, 0.7, Vec::new()).1;
        assert!(
            painted_label_center(&output, strings::HOME_OPEN).is_some(),
            "右键落在「更多」上也应弹出同一个行菜单"
        );
    }

    #[test]
    fn an_open_context_menu_keeps_its_row_hovered() {
        let mut app = HomeApp::new();
        app.set_boards(vec![board(7, 10)]);
        app.set_now(20);
        let ctx = egui::Context::default();
        let output = row_context_menu_frame(&ctx, &mut app);
        let delete = painted_label_center(&output, strings::HOME_DELETE).expect("菜单应该开着");
        // 指针挪到菜单里，那一行仍要留着悬停底。
        let (_, output) =
            frame_output(&ctx, &mut app, 0.6, vec![egui::Event::PointerMoved(delete)]);
        let row_hovered = output.shapes.iter().any(|clipped| {
            matches!(&clipped.shape, egui::Shape::Rect(rect)
                if rect.fill == theme::chrome().hover
                    && (rect.rect.height() - ROW_HEIGHT).abs() < 0.5
                    && rect.rect.width() > 1000.0)
        });
        assert!(row_hovered, "右键菜单开着时，那一行要保持悬停底");
    }

    #[test]
    fn the_context_menu_delete_sends_the_same_action_as_the_more_menu() {
        let mut app = HomeApp::new();
        app.set_boards(vec![board(7, 10)]);
        app.set_now(20);
        let ctx = egui::Context::default();
        let output = row_context_menu_frame(&ctx, &mut app);
        let delete = painted_label_center(&output, strings::HOME_DELETE).expect("菜单应该开着");
        let action = primary_click(&ctx, &mut app, 0.6, delete);
        assert_eq!(action, Some(HomeAction::Delete(7)));
    }

    #[test]
    fn secondary_click_below_the_rows_offers_a_new_note() {
        let mut app = HomeApp::new();
        app.set_boards(vec![board(7, 10)]);
        app.set_now(20);
        let ctx = egui::Context::default();
        // 第一行下面是空白：那里右键只给新建，不给笔记行菜单。
        let at = pos2(100.0, TOP_BAR_HEIGHT + ROW_HEIGHT + 20.0);
        secondary_click(&ctx, &mut app, 0.0, at);
        let output = frame_output(&ctx, &mut app, 0.5, Vec::new()).1;
        assert!(
            painted_label_center(&output, strings::HOME_DELETE).is_none(),
            "行下面的空白处不该弹出笔记行菜单"
        );
        let new_note = painted_label_center(&output, strings::SIDEBAR_NEW)
            .expect("空白处右键应弹出「新建笔记」");
        let action = primary_click(&ctx, &mut app, 0.6, new_note);
        assert_eq!(action, Some(HomeAction::Create));
    }

    #[test]
    fn an_empty_sidebar_also_right_clicks_to_create() {
        let mut app = HomeApp::new();
        let ctx = egui::Context::default();
        let at = pos2(100.0, TOP_BAR_HEIGHT + 40.0);
        secondary_click(&ctx, &mut app, 0.0, at);
        let output = frame_output(&ctx, &mut app, 0.5, Vec::new()).1;
        let new_note = painted_label_center(&output, strings::SIDEBAR_NEW)
            .expect("空列表里右键也应弹出「新建笔记」");
        let action = primary_click(&ctx, &mut app, 0.6, new_note);
        assert_eq!(action, Some(HomeAction::Create));
    }
}
