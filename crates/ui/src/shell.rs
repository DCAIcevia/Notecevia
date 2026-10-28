#![forbid(unsafe_code)]

//! 外壳：顶栏、左侧栏、两侧可收起的面板，把各块摆到各自的位置上。
//!
//! 这里只管位置和外观，不碰数据。要改数据的事都装进 `ShellOutput` 交回调用方。

use egui::{Align2, CornerRadius, FontId, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};
use notecevia_core::BlockKind;

use crate::board::BoardApp;
use crate::home::{HomeAction, HomeApp};
use crate::icons::{self, Icon};
use crate::layout::{self, Layout};
use crate::library::{Library, LibraryAction};
use crate::media_panel::{MediaAction, MediaPanel};
use crate::strings;
use crate::theme;
use crate::trash::{TrashAction, TrashView};

/// 顶栏高。外壳里所有面板的头部都跟它等高，横线才对得齐。
pub const TOP_BAR_HEIGHT: f32 = 48.0;
/// 全局工具栏高。横贯整窗，放在菜单栏下面。
pub const TOOLBAR_HEIGHT: f32 = 44.0;
/// 最左图标条宽。
pub const RAIL_WIDTH: f32 = 52.0;
/// 图标条里按钮的边长与圆角。
const RAIL_BUTTON: f32 = 36.0;
/// 图标在按钮里留多少边：36 的按钮画 20 的图标，纯图标的栏不能再小。
const RAIL_ICON_PAD: f32 = 8.0;
const RAIL_RADIUS: u8 = 8;
/// 顶栏左上角那个标记的边长。
const MARK: f32 = 22.0;
/// 面板边界上能抓住的那条宽。抓区偏向外侧，别压住栏里的滚动条。
const GRAB: f32 = 6.0;

/// 菜单栏按出来的命令。界面自己处理不了的，都交回 app。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuCommand {
    NewNote,
    ExportImage,
    ExportHtml,
    ExportContainer,
    CloseNote,
    Undo,
    Redo,
    DeleteSelection,
    SelectAll,
    ClearSelection,
    ToggleList,
    ToggleAssets,
    Quit,
    About,
}

/// 左侧栏的三层视图：一篇笔记 / 一组笔记 / 所有组，外加底部的回收站工具。
///
/// 现在只有「页」有数据；「册」与「书架」是分类管理，等模型定了再接上。
/// 回收站不是一层视图，是工具入口，借同一个状态位说"现在主区画的是谁"。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Page,
    Volume,
    Shelf,
    Trash,
}

/// 全局工具栏上的动作。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolCommand {
    /// 插入一块。
    Insert(notecevia_core::BlockKind),
    /// 把当前画布导成图片。
    ExportImage,
}

/// 外壳画完之后交回给调用方的东西。
#[derive(Default)]
pub struct ShellOutput {
    /// 左侧边栏按出来的动作。
    pub home: Option<HomeAction>,
    /// 点了"新建笔记"。
    pub new_note: bool,
    /// 全局工具栏上按出来的动作。
    pub tool: Option<ToolCommand>,
    /// 图标条上换了视图。
    pub view: Option<ViewMode>,
    /// 册视图按出来的动作。
    pub library: Option<LibraryAction>,
    /// 回收站里按出来的动作。
    pub trash: Option<TrashAction>,
    /// 资源栏里按出来的动作。
    pub media: Option<MediaAction>,
    /// 菜单栏按出来的命令。
    pub menu: Option<MenuCommand>,
    /// 面板宽度定下来了，该写回文件。只在松手之后给一次。
    pub layout: Option<Layout>,
}

/// 外壳自己的界面状态：搜索词、两块的宽度与当前视图。与笔记数据无关。
pub struct Shell {
    query: String,
    /// 两条面板的宽度，0 表示收起。拖过就记进 ui.json。
    layout: Layout,
    /// 宽度改过、还没写回文件。
    layout_dirty: bool,
    /// 正在拖的那条边界。拖的时候只跟指针走，松手才决定收不收。
    drag: Option<EdgeDrag>,
    view: ViewMode,
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl Shell {
    pub fn new() -> Self {
        Self::with_layout(Layout::default())
    }

    /// 用存下来的宽度起外壳。文件里没有就按默认。
    pub fn with_layout(layout: Layout) -> Self {
        Self {
            query: String::new(),
            layout,
            layout_dirty: false,
            drag: None,
            view: ViewMode::Page,
        }
    }

    /// 由 app 指定视图（例如从目录点开一页时切回页视图）。
    pub fn set_view(&mut self, view: ViewMode) {
        self.view = view;
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        home: &mut HomeApp,
        mut library: Option<&mut Library>,
        trash: &mut TrashView,
        media_panel: &mut MediaPanel,
        mut board: Option<&mut BoardApp>,
    ) -> ShellOutput {
        let mut output = ShellOutput::default();
        let full = ui.max_rect();
        let painter = ui.painter().clone();
        painter.rect_filled(full, CornerRadius::ZERO, theme::chrome().window);

        let top = Rect::from_min_size(full.min, Vec2::new(full.width(), TOP_BAR_HEIGHT));
        self.top_bar(ui, top, board.is_some(), &mut output);
        // 视图菜单只翻外壳自己的开关，不用惊动 app。宽度一改就要落盘。
        match output.menu {
            Some(MenuCommand::ToggleList) => {
                self.layout.list = toggle(self.layout.list, layout::LIST_DEFAULT);
                self.layout_dirty = true;
                output.menu = None;
            }
            Some(MenuCommand::ToggleAssets) => {
                self.layout.assets = toggle(self.layout.assets, layout::ASSETS_DEFAULT);
                self.layout_dirty = true;
                output.menu = None;
            }
            _ => {}
        }
        painter.line_segment(
            [top.left_bottom(), top.right_bottom()],
            Stroke::new(1.0, theme::chrome().line),
        );

        let body = Rect::from_min_max(egui::pos2(full.min.x, top.max.y), full.max);

        // 左侧栏：三层视图排一列。册与书架还没有数据，画淡、点了只说明。
        let rail = Rect::from_min_size(body.min, Vec2::new(RAIL_WIDTH, body.height()));
        painter.rect_filled(rail, CornerRadius::ZERO, theme::chrome().rail);
        painter.line_segment(
            [rail.right_top(), rail.right_bottom()],
            Stroke::new(1.0, theme::chrome().line_soft),
        );
        let entries = [
            (ViewMode::Page, Icon::Page, strings::SHELL_VIEW_PAGE, true),
            (
                ViewMode::Volume,
                Icon::Volume,
                strings::SHELL_VIEW_VOLUME,
                true,
            ),
            (
                ViewMode::Shelf,
                Icon::Shelf,
                strings::SHELL_VIEW_SHELF,
                false,
            ),
        ];
        for (index, (mode, icon, label, ready)) in entries.into_iter().enumerate() {
            let rect = Rect::from_center_size(
                egui::pos2(
                    rail.center().x,
                    rail.min.y
                        + theme::ui().gap_m
                        + RAIL_BUTTON / 2.0
                        + index as f32 * (RAIL_BUTTON + 2.0),
                ),
                Vec2::splat(RAIL_BUTTON),
            );
            if self.rail_button(ui, rect, icon, label, self.view == mode, ready) && ready {
                self.view = mode;
                output.view = Some(mode);
            }
        }
        // 图标条底部是工具入口，和上面的三层视图分开：回收站放这儿。
        // 设置等真有设置项再放——没有东西可设的入口只会占位置。
        let tool = Rect::from_center_size(
            egui::pos2(
                rail.center().x,
                rail.max.y - theme::ui().gap_m - RAIL_BUTTON / 2.0,
            ),
            Vec2::splat(RAIL_BUTTON),
        );
        if self.rail_button(
            ui,
            tool,
            Icon::Trash,
            strings::TRASH_TITLE,
            self.view == ViewMode::Trash,
            true,
        ) {
            self.view = ViewMode::Trash;
            output.view = Some(ViewMode::Trash);
        }

        // 两条面板的边界就是把手：拖动改宽度，拖到底收起，从收起里拖回来重新打开。
        // 抓区先登记，栏里的控件就能压在它上面——贴着内边缘的滚动条不该被边界抢走。
        // 窗口窄了就按可用宽度收一收：画的宽度与抓取的位置都取收过的值，存的值不动。
        let (list_width, assets_width) = layout::fit(
            self.layout.list,
            self.layout.assets,
            body.width() - RAIL_WIDTH,
        );
        let list_edge = rail.max.x + list_width;
        let assets_edge = body.max.x - assets_width;
        let list_grab = grab_strip(ui, body, list_edge, Side::Left, "shell-grab-list");
        let assets_grab = (self.view == ViewMode::Page)
            .then(|| grab_strip(ui, body, assets_edge, Side::Right, "shell-grab-assets"));
        let list_grab_held = list_grab.hovered() || list_grab.dragged();
        self.track_edge(
            &list_grab,
            Side::Left,
            list_width,
            layout::LIST_MIN,
            layout::LIST_MAX,
        );
        if list_grab.double_clicked() {
            self.layout.list = layout::LIST_DEFAULT;
            self.layout_dirty = true;
        }
        let mut assets_grab_held = false;
        if let Some(grab) = &assets_grab {
            assets_grab_held = grab.hovered() || grab.dragged();
            self.track_edge(
                grab,
                Side::Right,
                assets_width,
                layout::ASSETS_MIN,
                layout::ASSETS_MAX,
            );
            if grab.double_clicked() {
                self.layout.assets = layout::ASSETS_DEFAULT;
                self.layout_dirty = true;
            }
        }
        // 收起的那条没有自己的分隔线，悬停时补一条：不然没人知道这儿能拉出来。
        for (width, edge, held) in [
            (list_width, list_edge, list_grab_held),
            (assets_width, assets_edge, assets_grab_held),
        ] {
            if width > 0.0 || !held {
                continue;
            }
            painter.line_segment(
                [egui::pos2(edge, body.min.y), egui::pos2(edge, body.max.y)],
                boundary_stroke(true),
            );
        }

        // 左侧边栏：笔记列表，由 HomeApp 画内容。
        let mut main_left = rail.max.x;
        if list_width > 0.0 {
            let list = Rect::from_min_size(
                egui::pos2(rail.max.x, body.min.y),
                Vec2::new(list_width, body.height()),
            );
            painter.rect_filled(list, CornerRadius::ZERO, theme::chrome().panel);
            painter.line_segment(
                [list.right_top(), list.right_bottom()],
                boundary_stroke(list_grab_held),
            );
            match self.view {
                ViewMode::Volume => {
                    if let Some(library) = library.as_mut() {
                        ui.scope_builder(UiBuilder::new().max_rect(list), |ui| {
                            output.library = library.sidebar(ui);
                        });
                    }
                }
                _ => {
                    ui.scope_builder(UiBuilder::new().max_rect(list), |ui| {
                        output.home = home.sidebar(ui, &mut self.query);
                    });
                }
            }
            main_left = list.max.x;
        }

        // 右侧资源栏：整个工作区的媒体，只有页视图才有它。
        let mut main_right = body.max.x;
        if self.view == ViewMode::Page && assets_width > 0.0 {
            let panel = Rect::from_min_size(
                egui::pos2(body.max.x - assets_width, body.min.y),
                Vec2::new(assets_width, body.height()),
            );
            painter.rect_filled(panel, CornerRadius::ZERO, theme::chrome().panel);
            painter.line_segment(
                [panel.left_top(), panel.left_bottom()],
                boundary_stroke(assets_grab_held),
            );
            if let Some(action) = media_panel.ui(ui, panel) {
                output.media = Some(action);
            }
            main_right = panel.min.x;
        }

        // 主区：有笔记就画笔记，没有就给起始态。
        let main = Rect::from_min_max(
            egui::pos2(main_left, body.min.y),
            egui::pos2(main_right, body.max.y),
        );
        match self.view {
            ViewMode::Volume => {
                if let Some(library) = library.as_mut() {
                    ui.scope_builder(UiBuilder::new().max_rect(main), |ui| {
                        if let Some(action) = library.pages_view(ui) {
                            output.library = Some(action);
                        }
                    });
                }
            }
            ViewMode::Trash => {
                if let Some(action) = trash.ui(ui, main) {
                    output.trash = Some(action);
                }
            }
            _ => {
                if let Some(board) = board.as_mut() {
                    // 工具栏属于画布：对整块画布全局，不当标签栏，也不横跨侧栏。
                    let toolbar =
                        Rect::from_min_size(main.min, Vec2::new(main.width(), TOOLBAR_HEIGHT));
                    let notice = board.notice().map(|(text, error)| (text.to_owned(), error));
                    self.toolbar(ui, toolbar, notice, Some(&mut *board));
                    let canvas =
                        Rect::from_min_max(egui::pos2(main.min.x, toolbar.max.y), main.max);
                    ui.scope_builder(UiBuilder::new().max_rect(canvas), |ui| board.ui(ui));
                } else {
                    self.start_state(ui, main, &mut output);
                }
            }
        }
        // 拖动中不写文件：每帧都写既慢也没意义，松手才算摆完了。
        if self.layout_dirty && !ui.ctx().input(|input| input.pointer.any_down()) {
            self.layout.settle();
            self.layout_dirty = false;
            output.layout = Some(self.layout);
        }
        output
    }

    /// 跟一条边界的手势：按下记起点，拖动只跟指针走，松手才落定。
    ///
    /// 这样拖到最小宽度就停住，面板不会在手里突然消失；要收起得再往里拖一截，
    /// 松手才真的收起来。
    fn track_edge(&mut self, grab: &egui::Response, side: Side, start: f32, min: f32, max: f32) {
        let grow = match side {
            Side::Left => 1.0,
            Side::Right => -1.0,
        };
        if grab.drag_started() {
            self.drag = Some(EdgeDrag {
                side,
                start,
                want: start,
            });
        }
        let Some(mut drag) = self.drag.filter(|drag| drag.side == side) else {
            return;
        };
        if grab.dragged() {
            drag.want += grab.drag_delta().x * grow;
            self.drag = Some(drag);
            self.store_width(side, layout::dragging(drag.start, drag.want, min, max));
            self.layout_dirty = true;
        }
        if grab.drag_stopped() {
            self.drag = None;
            self.store_width(side, layout::released(drag.start, drag.want, min, max));
            self.layout_dirty = true;
        }
    }

    /// 把宽度写到对应的那条栏上。
    fn store_width(&mut self, side: Side, width: f32) {
        match side {
            Side::Left => self.layout.list = width,
            Side::Right => self.layout.assets = width,
        }
    }

    /// 顶栏：左边是标记，接着是菜单。右边留白，菜单不顶到窗口边上。
    fn top_bar(&mut self, ui: &mut Ui, rect: Rect, has_note: bool, output: &mut ShellOutput) {
        let painter = ui.painter().clone();
        // 标记落在图标条那一列里：和下面那条竖栏是一条线。
        let mark = Rect::from_center_size(
            egui::pos2(rect.min.x + RAIL_WIDTH / 2.0, rect.center().y),
            Vec2::splat(MARK),
        );
        painter.rect_filled(mark, CornerRadius::same(6), theme::chrome().text);
        // 近黑方块里一个白 n，和窗口图标同一个记号。不另放图片，放大缩小都不糊。
        icons::draw(
            &painter,
            mark.shrink(5.0),
            Icon::Mark,
            theme::chrome().window,
        );

        // 菜单与左栏里的内容同一条竖线：对齐表示它们是一路的。
        // 竖向按一行字算：上下都居中，别贴着栏顶。
        // 一行菜单按钮实际多高：按钮字高加上下内边距，照着它居中就不会贴顶。
        let row =
            ui.text_style_height(&egui::TextStyle::Button) + ui.spacing().button_padding.y * 2.0;
        let left = rect.min.x + RAIL_WIDTH + theme::ui().gap_m;
        let menus = Rect::from_min_max(
            egui::pos2(left, rect.center().y - row / 2.0),
            egui::pos2(rect.max.x - theme::ui().gap_m, rect.center().y + row / 2.0),
        );
        self.menu_bar(ui, menus, has_note, output);
    }

    /// 全局工具栏：一条横贯整窗的插入工具条，右侧是缩放与导出。
    ///
    /// 标题不放这儿：一条栏里顶着文档名，看着就像标签页；名字交给窗口标题。
    fn toolbar(
        &self,
        ui: &mut Ui,
        rect: Rect,
        notice: Option<(String, bool)>,
        mut board: Option<&mut BoardApp>,
    ) {
        let painter = ui.painter().clone();
        painter.rect_filled(rect, CornerRadius::ZERO, theme::chrome().window);
        // 工具栏与画布之间要看得见一条界：太淡就等于没有工具栏。
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, theme::chrome().line),
        );
        let control = theme::ui().control_height;
        let top = rect.center().y - control / 2.0;
        let mut x = rect.min.x + theme::ui().gap_m;

        for (icon, label, kind) in [
            (Icon::Text, strings::NEW_TEXT, BlockKind::Text),
            (Icon::Table, strings::NEW_TABLE, BlockKind::Table),
            (Icon::Shape, strings::NEW_SHAPE, BlockKind::Shape),
            (
                Icon::Highlight,
                strings::NEW_HIGHLIGHT,
                BlockKind::Highlight,
            ),
        ] {
            let button = Rect::from_min_size(egui::pos2(x, top), Vec2::splat(control));
            if self.tool_button(ui, button, icon, label)
                && let Some(board) = board.as_mut()
            {
                board.insert_kind(kind);
            }
            x += control + theme::ui().gap_s;
        }

        let right = rect.max.x - theme::ui().gap_m;

        if let Some((text, error)) = notice {
            let colour = if error {
                theme::chrome().danger
            } else {
                theme::chrome().text_faint
            };
            painter.text(
                egui::pos2(right, rect.center().y),
                Align2::RIGHT_CENTER,
                text,
                FontId::proportional(theme::ui().meta),
                colour,
            );
        }
    }

    /// 工具栏上的图标按钮：悬停与按下只换底色。
    fn tool_button(&self, ui: &mut Ui, rect: Rect, icon: Icon, label: &str) -> bool {
        let response = ui.interact(rect, ui.scope_id().with(label), Sense::click());
        let painter = ui.painter();
        let active = response.has_focus() || response.is_pointer_button_down_on();
        if active {
            painter.rect_filled(
                rect,
                CornerRadius::same(theme::ui().radius_control),
                theme::chrome().pressed,
            );
        } else if response.hovered() {
            painter.rect_filled(
                rect,
                CornerRadius::same(theme::ui().radius_control),
                theme::chrome().hover,
            );
        }
        let colour = if response.hovered() || active {
            theme::chrome().text
        } else {
            theme::chrome().text_muted
        };
        // 工具栏的图标画 20，比按钮本身小一圈就够，别再缩。
        icons::draw(painter, rect.shrink(4.0), icon, colour);
        response.on_hover_text(label).clicked()
    }

    /// 菜单栏：文件 / 编辑 / 选择 / 视图 / 帮助。一条老派的栏。
    fn menu_bar(&self, ui: &mut Ui, rect: Rect, has_note: bool, output: &mut ShellOutput) {
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            // 菜单项之间拉开一点，别挤成一坨字。
            ui.spacing_mut().item_spacing.x = theme::ui().gap_l;
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(strings::MENU_FILE, |ui| {
                    if menu_item(ui, strings::MENU_NEW_NOTE, &shortcut("N"), true) {
                        output.menu = Some(MenuCommand::NewNote);
                    }
                    ui.separator();
                    if menu_item(ui, strings::EXPORT_IMAGE, "", has_note) {
                        output.menu = Some(MenuCommand::ExportImage);
                    }
                    if menu_item(ui, strings::MENU_EXPORT_HTML, "", has_note) {
                        output.menu = Some(MenuCommand::ExportHtml);
                    }
                    if menu_item(ui, strings::MENU_EXPORT_CONTAINER, "", has_note) {
                        output.menu = Some(MenuCommand::ExportContainer);
                    }
                    ui.separator();
                    if menu_item(ui, strings::MENU_CLOSE_NOTE, &shortcut("H"), has_note) {
                        output.menu = Some(MenuCommand::CloseNote);
                    }
                    ui.separator();
                    if menu_item(ui, strings::MENU_QUIT, "", true) {
                        output.menu = Some(MenuCommand::Quit);
                    }
                });
                ui.menu_button(strings::MENU_EDIT, |ui| {
                    if menu_item(ui, strings::MENU_UNDO, &shortcut("Z"), has_note) {
                        output.menu = Some(MenuCommand::Undo);
                    }
                    if menu_item(ui, strings::MENU_REDO, &shortcut("Shift Z"), has_note) {
                        output.menu = Some(MenuCommand::Redo);
                    }
                    ui.separator();
                    if menu_item(ui, strings::MENU_DELETE, "Delete", has_note) {
                        output.menu = Some(MenuCommand::DeleteSelection);
                    }
                });
                ui.menu_button(strings::MENU_SELECT, |ui| {
                    if menu_item(ui, strings::MENU_SELECT_ALL, &shortcut("A"), has_note) {
                        output.menu = Some(MenuCommand::SelectAll);
                    }
                    if menu_item(ui, strings::MENU_CLEAR_SELECT, "Esc", has_note) {
                        output.menu = Some(MenuCommand::ClearSelection);
                    }
                });
                ui.menu_button(strings::MENU_VIEW, |ui| {
                    // 文案跟着当前状态走：按钮收起来了，菜单上就该写着展开。
                    let list = if self.layout.list_open() {
                        strings::MENU_HIDE_LIST
                    } else {
                        strings::MENU_SHOW_LIST
                    };
                    if menu_item(ui, list, "", true) {
                        output.menu = Some(MenuCommand::ToggleList);
                    }
                    let assets = if self.layout.assets_open() {
                        strings::MENU_HIDE_ASSETS
                    } else {
                        strings::MENU_SHOW_ASSETS
                    };
                    if menu_item(ui, assets, "", true) {
                        output.menu = Some(MenuCommand::ToggleAssets);
                    }
                });
                ui.menu_button(strings::MENU_HELP, |ui| {
                    if menu_item(ui, strings::MENU_ABOUT, "", true) {
                        output.menu = Some(MenuCommand::About);
                    }
                });
            });
        });
    }

    /// 没有打开的笔记时，主区只给一件事：新建。
    fn start_state(&self, ui: &mut Ui, rect: Rect, output: &mut ShellOutput) {
        // 只画在主区里：主区窄的时候宁可截断，也不能压到两边的栏上。
        let painter = ui.painter().clone().with_clip_rect(rect);
        let anchor = egui::pos2(rect.center().x, rect.min.y + rect.height() * 0.36);
        painter.text(
            egui::pos2(anchor.x, anchor.y),
            Align2::CENTER_CENTER,
            strings::SHELL_NO_NOTE,
            FontId::proportional(theme::ui().empty),
            theme::chrome().text_muted,
        );
        let button = Rect::from_center_size(
            egui::pos2(anchor.x, anchor.y + 40.0),
            Vec2::new(96.0, theme::ui().control_height + 4.0),
        );
        let response = ui.interact(button, ui.scope_id().with("shell-new-note"), Sense::click());
        let fill = if response.hovered() {
            egui::Color32::from_rgb(40, 40, 44)
        } else {
            theme::chrome().text
        };
        painter.rect_filled(button, CornerRadius::same(theme::ui().radius_control), fill);
        painter.text(
            button.center(),
            Align2::CENTER_CENTER,
            strings::SHELL_NEW_NOTE,
            FontId::proportional(theme::ui().body),
            theme::chrome().window,
        );
        if response.clicked() {
            output.new_note = true;
        }
    }

    /// 左侧栏的一个入口：选中的出底；没做的画淡，点了只在悬停里说明还没做。
    fn rail_button(
        &self,
        ui: &mut Ui,
        rect: Rect,
        icon: Icon,
        label: &str,
        selected: bool,
        ready: bool,
    ) -> bool {
        let response = ui.interact(rect, ui.scope_id().with(label), Sense::click());
        let painter = ui.painter();
        if selected {
            painter.rect_filled(
                rect,
                CornerRadius::same(RAIL_RADIUS),
                theme::chrome().pressed,
            );
        } else if ready && response.hovered() {
            painter.rect_filled(rect, CornerRadius::same(RAIL_RADIUS), theme::chrome().hover);
        }
        let color = if !ready {
            theme::chrome().text_faint
        } else if selected || response.hovered() {
            theme::chrome().text
        } else {
            theme::chrome().text_muted
        };
        icons::draw(painter, rect.shrink(RAIL_ICON_PAD), icon, color);
        let tip = if ready {
            label.to_owned()
        } else {
            format!("{label}：{}", strings::SHELL_NOT_YET)
        };
        response.on_hover_text(tip).clicked()
    }
}

/// 面板边界上的抓取区。偏向外侧多让几像素：收起的栏也得抓得着。
fn grab_strip(ui: &mut Ui, body: Rect, edge: f32, side: Side, id: &str) -> egui::Response {
    let (left, right) = match side {
        Side::Left => (edge - 1.0, edge + GRAB - 1.0),
        Side::Right => (edge - GRAB + 1.0, edge + 1.0),
    };
    let rect = Rect::from_min_max(egui::pos2(left, body.min.y), egui::pos2(right, body.max.y));
    let response = ui.interact(rect, ui.scope_id().with(id), Sense::click_and_drag());
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    response.on_hover_text(strings::SHELL_RESIZE)
}

/// 面板边界那条线：平时是普通分隔线，指针落在抓取区里就亮一档。
///
/// 只换颜色不加粗：边界的位置不能因为悬停而变。
fn boundary_stroke(active: bool) -> Stroke {
    if active {
        Stroke::new(1.0, theme::chrome().text_faint)
    } else {
        Stroke::new(1.0, theme::chrome().line)
    }
}

/// 菜单里的收起与展开：宽度是 0 就是收起。
fn toggle(width: f32, default: f32) -> f32 {
    if width > 0.0 { 0.0 } else { default }
}

/// 边界在哪一侧。抓取区往栏外让，收起时也能抓到。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

/// 正在拖的那条边界：起点宽度与指针累计想去的宽度。
#[derive(Clone, Copy)]
struct EdgeDrag {
    side: Side,
    /// 按下时这条栏的宽度。收起是 0。
    start: f32,
    /// 累计到这一刻想要的宽度，不受范围限制：判断"拖过头了"靠它。
    want: f32,
}

/// 菜单里一项：动作 + 快捷键提示。禁用的项照样显示，只是点不动。
fn menu_item(ui: &mut Ui, label: &str, shortcut: &str, enabled: bool) -> bool {
    let mut button = egui::Button::new(label);
    if !shortcut.is_empty() {
        button = button.shortcut_text(shortcut);
    }
    ui.add_enabled(enabled, button).clicked()
}

/// 快捷键提示按平台写：mac 用 ⌘，其余用 Ctrl。
fn shortcut(key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘ {key}")
    } else {
        format!("Ctrl {key}")
    }
}
