#![forbid(unsafe_code)]

//! 首页：画板列表、搜索、标签与导出入口。
//!
//! 首页只产出一个动作，由 app 执行真正的读写。这样界面层不用碰文件。

use std::time::{SystemTime, UNIX_EPOCH};

use egui::{
    Align, Align2, Button, CornerRadius, FontId, Key, Label, Layout, Margin, Rect, RichText, Sense,
    Shape, TextEdit, TextWrapMode, Ui, pos2, text::LayoutJob, text::TextFormat, vec2,
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

/// 首页状态：列表、搜索词、待确认的删除、正在编辑的标题。
pub struct HomeApp {
    boards: Vec<BoardItem>,
    state: HomeState,
}

/// 跟列表内容无关的交互状态。
#[derive(Default)]
struct HomeState {
    search: String,
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
        Self {
            boards: Vec::new(),
            state: HomeState::default(),
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
        let Self { boards, state } = self;
        state.typing = false;

        egui::Frame::new()
            .inner_margin(Margin::symmetric(PAGE_MARGIN_X, PAGE_MARGIN_Y))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if let Some(chosen) = top_bar(state, ui, &palette) {
                    action = Some(chosen);
                }
                status_line(state, ui, &palette);
                ui.add_space(8.0);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if boards.is_empty() {
                            if let Some(chosen) = placeholder(state, ui, &palette)
                                && action.is_none()
                            {
                                action = Some(chosen);
                            }
                            return;
                        }
                        let now = now_secs();
                        for board in boards.iter() {
                            if let Some(chosen) = board_row(state, ui, &palette, board, now)
                                && action.is_none()
                            {
                                action = Some(chosen);
                            }
                            ui.add_space(ROW_GAP);
                        }
                    });

                handle_keys(state, boards, ui, &mut action);
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

/// 空态与无结果态。两种情况都给下一步。
fn placeholder(state: &mut HomeState, ui: &mut Ui, palette: &theme::Palette) -> Option<HomeAction> {
    let searching = !state.search.trim().is_empty();
    let mut action = None;
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        let (title, hint) = if searching {
            (strings::HOME_NO_RESULTS, strings::HOME_NO_RESULTS_HINT)
        } else {
            (strings::HOME_EMPTY, strings::HOME_EMPTY_HINT)
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
        // 无结果时下一步是改词或清空搜索：搜索框右侧的 × 就在这一屏里。
        if !searching && ui.button(strings::HOME_NEW).clicked() {
            action = Some(HomeAction::Create);
        }
    });
    action
}

fn board_row(
    state: &mut HomeState,
    ui: &mut Ui,
    palette: &theme::Palette,
    board: &BoardItem,
    now: i64,
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
                    ui.horizontal(|ui| {
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
    boards: &[BoardItem],
    ui: &Ui,
    action: &mut Option<HomeAction>,
) {
    // 新建不挑焦点，任何时候都能按。
    if ui.input(|input| input.modifiers.command && input.key_pressed(Key::N)) {
        *action = Some(HomeAction::Create);
        return;
    }
    // 在文本框里打字时，方向键是移光标，不是选行。
    if state.typing || state.editing.is_some() || boards.is_empty() {
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
}
