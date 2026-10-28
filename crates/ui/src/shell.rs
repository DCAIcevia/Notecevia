#![forbid(unsafe_code)]

//! 外壳：顶栏、左侧栏、两侧可收起的面板，把各块摆到各自的位置上。
//!
//! 这里只管位置和外观，不碰数据。要改数据的事都装进 `ShellOutput` 交回调用方。

use egui::{Align2, CornerRadius, FontId, Rect, Sense, Ui, UiBuilder, Vec2};
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
use crate::window::WindowHooks;

/// 顶栏高。外壳里所有面板的头部都跟它等高，横线才对得齐。
pub const TOP_BAR_HEIGHT: f32 = 48.0;
/// 顶部那条岛的高度。微软的标题栏规范是 32，图标上下各留 8。
const HEADER_HEIGHT: f32 = 32.0;
/// 窗口按钮的宽度。系统标题栏按钮是 46×32，这里等比照搬。
const WINDOW_BUTTON: f32 = 46.0;
/// 窗口图标的边长与到左边界的距离。
const MARK: f32 = 16.0;
const MARK_INSET: f32 = 16.0;
/// 标题栏里元素之间的间隔。
const HEADER_GAP: f32 = 16.0;
/// 岛与岛之间的间隙。分界全靠露出来的底色，不画线。
const ISLAND_GAP: f32 = 6.0;
/// 窗口边缘到岛的留白。
const ISLAND_MARGIN: f32 = 6.0;
/// 窗口四个角的圆角。Windows 11 的窗口是 8，这里跟着系统走。
const WINDOW_RADIUS: f32 = 8.0;
/// 全局工具栏高。横贯整窗，放在菜单栏下面。
pub const TOOLBAR_HEIGHT: f32 = 44.0;
/// 最左图标条宽。
pub const RAIL_WIDTH: f32 = 52.0;
/// 图标条里按钮的边长与圆角。
const RAIL_BUTTON: f32 = 36.0;
/// 图标在按钮里留多少边：36 的按钮画 20 的图标，纯图标的栏不能再小。
const RAIL_ICON_PAD: f32 = 8.0;
const RAIL_RADIUS: u8 = 8;
/// 面板边界上能抓住的那条宽。抓区偏向外侧，别压住栏里的滚动条。
const GRAB: f32 = 6.0;

/// 顶栏右上角的三个窗口按钮。系统标题栏关掉之后由界面自己画。
#[derive(Clone, Copy, PartialEq, Eq)]
enum WindowButton {
    Minimize,
    Maximize,
    Restore,
    Close,
}

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

/// 一帧要画的那几块内容。外壳不拥有它们，只借用一帧。
///
/// 打成一个结构而不是一长串位置参数：调用点能写字段名，加一块内容也不用改签名。
pub struct Views<'a> {
    pub home: &'a mut HomeApp,
    pub library: Option<&'a mut Library>,
    pub trash: &'a mut TrashView,
    pub media_panel: &'a mut MediaPanel,
    pub board: Option<&'a mut BoardApp>,
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
    /// 正在拖窗口：按下时的窗口左上角与指针，都在屏幕坐标里。
    window_drag: Option<(egui::Pos2, egui::Pos2)>,
    /// 标题。系统标题栏没了，文档名只剩这一处能看见。
    title: String,
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
            window_drag: None,
            title: String::new(),
            view: ViewMode::Page,
        }
    }

    /// 由 app 指定视图（例如从目录点开一页时切回页视图）。
    pub fn set_view(&mut self, view: ViewMode) {
        self.view = view;
    }

    /// 标题栏上写什么。系统标题栏没了，这是文档名唯一看得见的地方。
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    pub fn ui(&mut self, ui: &mut Ui, hooks: &dyn WindowHooks, views: Views<'_>) -> ShellOutput {
        let Views {
            home,
            mut library,
            trash,
            media_panel,
            mut board,
        } = views;
        let mut output = ShellOutput::default();
        let full = ui.max_rect();
        let painter = ui.painter().clone();
        let radius = CornerRadius::same(theme::ui().radius_surface);
        // 岛与岛之间露出来的那层底：分界全交给它，不再画线。
        // 圆角画在这张底上：窗口本身是方的，四角靠透明透出桌面。
        painter.rect_filled(
            full,
            CornerRadius::same(WINDOW_RADIUS as u8),
            theme::chrome().ground,
        );

        let header = Rect::from_min_max(
            full.min + Vec2::splat(ISLAND_MARGIN),
            egui::pos2(
                full.max.x - ISLAND_MARGIN,
                full.min.y + ISLAND_MARGIN + HEADER_HEIGHT,
            ),
        );
        self.resize_edges(ui, full);
        self.header(ui, header, board.is_some(), hooks, &mut output);
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
        let body = Rect::from_min_max(
            egui::pos2(full.min.x + ISLAND_MARGIN, header.max.y + ISLAND_GAP),
            egui::pos2(full.max.x - ISLAND_MARGIN, full.max.y - ISLAND_MARGIN),
        );

        // 左侧栏：三层视图排一列。册与书架还没有数据，画淡、点了只说明。
        let rail = Rect::from_min_size(body.min, Vec2::new(RAIL_WIDTH, body.height()));
        painter.rect_filled(rail, radius, theme::chrome().panel);
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
        let inner_left = rail.max.x + ISLAND_GAP;
        let (list_width, assets_width) = layout::fit(
            self.layout.list,
            self.layout.assets,
            body.max.x - inner_left,
        );
        // 抓区落在两岛之间的缝里：指针在缝上就是能拖。
        let list_edge = inner_left + list_width + ISLAND_GAP / 2.0;
        let assets_edge = body.max.x - assets_width - ISLAND_GAP / 2.0;
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
        // 缝本身就是把手：拖的时候把缝染深一点，不另外画线。
        for (edge, held) in [(list_edge, list_grab_held), (assets_edge, assets_grab_held)] {
            if !held {
                continue;
            }
            let seam = Rect::from_min_max(
                egui::pos2(edge - ISLAND_GAP / 2.0, body.min.y),
                egui::pos2(edge + ISLAND_GAP / 2.0, body.max.y),
            );
            painter.rect_filled(seam, CornerRadius::same(2), theme::chrome().pressed);
        }

        // 左侧边栏：笔记列表，由 HomeApp 画内容。
        let mut main_left = inner_left;
        if list_width > 0.0 {
            let list = Rect::from_min_size(
                egui::pos2(inner_left, body.min.y),
                Vec2::new(list_width, body.height()),
            );
            painter.rect_filled(list, radius, theme::chrome().panel);
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
            main_left = list.max.x + ISLAND_GAP;
        }

        // 右侧资源栏：整个工作区的媒体，只有页视图才有它。
        let mut main_right = body.max.x;
        if self.view == ViewMode::Page && assets_width > 0.0 {
            let panel = Rect::from_min_size(
                egui::pos2(body.max.x - assets_width, body.min.y),
                Vec2::new(assets_width, body.height()),
            );
            painter.rect_filled(panel, radius, theme::chrome().panel);
            if let Some(action) = media_panel.ui(ui, panel) {
                output.media = Some(action);
            }
            main_right = panel.min.x - ISLAND_GAP;
        }

        // 主区：有笔记就画笔记，没有就给起始态。
        let main = Rect::from_min_max(
            egui::pos2(main_left, body.min.y),
            egui::pos2(main_right, body.max.y),
        );
        // 主区是唯一一块白底的岛：内容是纸，纸比工具栏亮一档。
        painter.rect_filled(main, radius, theme::chrome().window);
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
                    // 留一圈内边距：内容不压在卡的圆角上，卡边才看得出是条边。
                    let canvas =
                        Rect::from_min_max(egui::pos2(main.min.x, toolbar.max.y), main.max)
                            .shrink2(Vec2::new(theme::ui().gap_m, theme::ui().gap_m));
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

    /// 顶栏：一条岛。从左到右是标记、菜单、窗口按钮，空白处能拖窗口。
    ///
    /// 系统标题栏已经关掉，所以最小化、最大化、关闭都在这里画：一层头，
    /// 品牌只出现一次。拖动与最大化优先交给平台层，平台不管才退回 egui 的命令。
    fn header(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        has_note: bool,
        hooks: &dyn WindowHooks,
        output: &mut ShellOutput,
    ) {
        let painter = ui.painter().clone();
        let chrome = theme::chrome();
        painter.rect_filled(
            rect,
            CornerRadius::same(theme::ui().radius_surface),
            chrome.panel,
        );

        let close = Rect::from_min_size(
            egui::pos2(rect.max.x - WINDOW_BUTTON, rect.min.y),
            Vec2::new(WINDOW_BUTTON, rect.height()),
        );
        let maximize = close.translate(Vec2::new(-WINDOW_BUTTON, 0.0));
        let minimize = maximize.translate(Vec2::new(-WINDOW_BUTTON, 0.0));

        // 先登记拖区，菜单随后压在它上面：拖窗口不会把菜单点掉。
        // 底边要取头部的下沿：用按钮的 min 会得到一条零高度的线，永远命中不了。
        let strip = Rect::from_min_max(rect.min, egui::pos2(minimize.min.x, rect.max.y));
        let drag = ui.interact(
            strip,
            ui.scope_id().with("shell-drag"),
            Sense::click_and_drag(),
        );
        // 拖动自己算：每帧按指针走了多少发一个位置命令。交给系统的移动循环更"原生"，
        // 但那段时间应用不再出帧，画面是冻的——这就是之前不跟手的原因。
        if drag.drag_started() {
            self.window_drag = grab_window(ui);
        }
        if drag.dragged()
            && let (Some((start, grab)), Some(pointer)) = (self.window_drag, screen_pointer(ui))
        {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::OuterPosition(
                    start + (pointer - grab),
                ));
        }
        if drag.drag_stopped() {
            self.window_drag = None;
        }
        // 微软的标题栏规范：没有交互元素的地方右键出系统窗口菜单。
        if drag.secondary_clicked()
            && let Some(pointer) = screen_pointer(ui)
        {
            hooks.show_system_menu(pointer.x, pointer.y);
        }
        if drag.double_clicked() && hooks.toggle_maximize().is_none() {
            let maximized = self.maximized(ui, hooks);
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }

        // 窗口图标：16 见方，离左边界 16，竖向居中——照系统标题栏的规格。
        let mark = Rect::from_center_size(
            egui::pos2(rect.min.x + MARK_INSET + MARK / 2.0, rect.center().y),
            Vec2::splat(MARK),
        );
        painter.rect_filled(mark, CornerRadius::same(4), chrome.text);
        // 近黑方块里一个白 n，和窗口图标同一个记号。不另放图片，放大缩小都不糊。
        icons::draw(&painter, mark.shrink(3.0), Icon::Mark, chrome.window);

        // 菜单跟在图标后面，间距 16。
        let row =
            ui.text_style_height(&egui::TextStyle::Button) + ui.spacing().button_padding.y * 2.0;
        let menus = Rect::from_min_max(
            egui::pos2(mark.right() + HEADER_GAP, rect.center().y - row / 2.0),
            egui::pos2(minimize.min.x - HEADER_GAP, rect.center().y + row / 2.0),
        );
        self.menu_bar(ui, menus, has_note, output);

        // 标题居中：菜单占了左边，窗口按钮占了右边，中间这块才是标题的地方。
        // 太长就按可用宽度截断，不让它顶到两边的控件上。
        let title_area = Rect::from_min_max(
            egui::pos2(menus.min.x, rect.min.y),
            egui::pos2(minimize.min.x, rect.max.y),
        );
        if !self.title.is_empty() {
            let colour = if ui
                .ctx()
                .input(|input| input.viewport().focused.unwrap_or(true))
            {
                chrome.text_muted
            } else {
                chrome.text_faint
            };
            painter.with_clip_rect(title_area).text(
                title_area.center(),
                Align2::CENTER_CENTER,
                &self.title,
                FontId::proportional(theme::ui().body),
                colour,
            );
        }

        let maximized = self.maximized(ui, hooks);
        self.window_button(ui, minimize, WindowButton::Minimize, hooks);
        self.window_button(
            ui,
            maximize,
            if maximized {
                WindowButton::Restore
            } else {
                WindowButton::Maximize
            },
            hooks,
        );
        self.window_button(ui, close, WindowButton::Close, hooks);
    }

    /// 现在是不是最大化。平台知道就问平台，不知道就问 egui。
    fn maximized(&self, ui: &Ui, hooks: &dyn WindowHooks) -> bool {
        hooks.is_maximized().unwrap_or_else(|| {
            ui.ctx()
                .input(|input| input.viewport().maximized.unwrap_or(false))
        })
    }

    /// 窗口按钮：图形自己画，不依赖字体里有没有那几个符号。
    fn window_button(&self, ui: &mut Ui, rect: Rect, kind: WindowButton, hooks: &dyn WindowHooks) {
        let response = ui.interact(
            rect,
            ui.scope_id().with(("window-button", kind as u8)),
            Sense::click(),
        );
        let chrome = theme::chrome();
        let closing = kind == WindowButton::Close;
        let focused = ui
            .ctx()
            .input(|input| input.viewport().focused.unwrap_or(true));
        // 悬停、按下、静置三档底色都取主题；系统标题栏按钮是整格出血的，
        // 这里只内缩 2——它坐在岛里，出血到边缘会把卡的圆角盖成方的。
        let fill = if response.is_pointer_button_down_on() {
            Some(if closing {
                chrome.danger
            } else {
                chrome.pressed
            })
        } else if response.hovered() {
            Some(if closing { chrome.danger } else { chrome.hover })
        } else {
            None
        };
        if let Some(fill) = fill {
            ui.painter().rect_filled(
                rect.shrink(2.0),
                CornerRadius::same(theme::ui().radius_control),
                fill,
            );
        }
        // 窗口失去焦点时按钮也要退一档，否则背景窗口的按钮看着仍然"可按"。
        let colour = if response.hovered() && closing {
            chrome.window
        } else if !focused {
            chrome.text_faint
        } else if response.hovered() {
            chrome.text
        } else {
            chrome.text_muted
        };
        paint_window_glyph(ui.painter(), rect, kind, colour);
        if !response.clicked() {
            return;
        }
        match kind {
            WindowButton::Minimize => {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            WindowButton::Maximize | WindowButton::Restore => {
                if hooks.toggle_maximize().is_none() {
                    let maximized = self.maximized(ui, hooks);
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                }
            }
            WindowButton::Close => {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// 岛的留白就是窗口的边框：贴着留白拖就是改窗口大小。
    ///
    /// 这几条窄带全在卡的圆角外面，所以不会抢走任何内容的点击。
    fn resize_edges(&self, ui: &mut Ui, full: Rect) {
        let band = ISLAND_MARGIN + 2.0;
        let corner = ISLAND_MARGIN + 6.0;
        let edges = [
            (
                Rect::from_min_max(
                    egui::pos2(full.min.x + band, full.min.y),
                    egui::pos2(full.max.x - band, full.min.y + band),
                ),
                egui::ResizeDirection::North,
                egui::CursorIcon::ResizeVertical,
            ),
            (
                Rect::from_min_max(
                    egui::pos2(full.min.x + band, full.max.y - band),
                    egui::pos2(full.max.x - band, full.max.y),
                ),
                egui::ResizeDirection::South,
                egui::CursorIcon::ResizeVertical,
            ),
            (
                Rect::from_min_max(
                    egui::pos2(full.min.x, full.min.y + band),
                    egui::pos2(full.min.x + band, full.max.y - band),
                ),
                egui::ResizeDirection::West,
                egui::CursorIcon::ResizeHorizontal,
            ),
            (
                Rect::from_min_max(
                    egui::pos2(full.max.x - band, full.min.y + band),
                    egui::pos2(full.max.x, full.max.y - band),
                ),
                egui::ResizeDirection::East,
                egui::CursorIcon::ResizeHorizontal,
            ),
        ];
        let corners = [
            (
                Rect::from_min_size(full.min, Vec2::splat(corner)),
                egui::ResizeDirection::NorthWest,
                egui::CursorIcon::ResizeNwSe,
            ),
            (
                Rect::from_min_size(
                    egui::pos2(full.max.x - corner, full.min.y),
                    Vec2::splat(corner),
                ),
                egui::ResizeDirection::NorthEast,
                egui::CursorIcon::ResizeNeSw,
            ),
            (
                Rect::from_min_size(
                    egui::pos2(full.min.x, full.max.y - corner),
                    Vec2::splat(corner),
                ),
                egui::ResizeDirection::SouthWest,
                egui::CursorIcon::ResizeNeSw,
            ),
            (
                Rect::from_min_size(
                    egui::pos2(full.max.x - corner, full.max.y - corner),
                    Vec2::splat(corner),
                ),
                egui::ResizeDirection::SouthEast,
                egui::CursorIcon::ResizeNwSe,
            ),
        ];
        // 角后登记：叠在边上时角优先。
        for (index, (rect, direction, cursor)) in edges.into_iter().chain(corners).enumerate() {
            let response = ui.interact(
                rect,
                ui.scope_id().with(("shell-resize", index)),
                Sense::click_and_drag(),
            );
            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(cursor);
            }
            // 缩放交给系统的 DRAG_RESIZE。它拖动期间会让画面停更，但尺寸由系统算，
            // 不会和我们自己发的尺寸命令互相打架——手算那次窗口被撑到 8000 多像素，
            // 直接把绘制后端顶崩了。宁可钝一点，不能崩。
            if response.hovered() && ui.ctx().input(|input| input.pointer.primary_pressed()) {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
            }
        }
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
        // 工具栏和画布同在一张白岛里：不再单独铺底，也不画横线。
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

/// 指针在屏幕上的位置。
///
/// 局部坐标加上窗口左上角：窗口跟着指针走的时候，只有屏幕坐标能用来算位移，
/// 用局部坐标会和窗口自己的移动互相抵消。
fn screen_pointer(ui: &Ui) -> Option<egui::Pos2> {
    let (local, inner) = ui
        .ctx()
        .input(|input| (input.pointer.interact_pos(), input.viewport().inner_rect));
    Some(inner?.min + egui::vec2(local?.x, local?.y))
}

/// 按下拖动时记下两个起点：窗口在哪、指针在哪。
fn grab_window(ui: &Ui) -> Option<(egui::Pos2, egui::Pos2)> {
    let outer = ui.ctx().input(|input| input.viewport().outer_rect)?;
    Some((outer.min, screen_pointer(ui)?))
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

/// 窗口按钮上那个图形：一条横线、一个小方框、一个叉。
fn paint_window_glyph(
    painter: &egui::Painter,
    rect: Rect,
    kind: WindowButton,
    colour: egui::Color32,
) {
    let centre = rect.center();
    let stroke = egui::Stroke::new(1.0, colour);
    match kind {
        WindowButton::Minimize => {
            painter.line_segment(
                [
                    egui::pos2(centre.x - 5.0, centre.y + 0.5),
                    egui::pos2(centre.x + 5.0, centre.y + 0.5),
                ],
                stroke,
            );
        }
        WindowButton::Maximize => {
            painter.rect_stroke(
                Rect::from_center_size(centre, Vec2::splat(10.0)),
                CornerRadius::ZERO,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        WindowButton::Restore => {
            painter.rect_stroke(
                Rect::from_center_size(
                    egui::pos2(centre.x + 1.5, centre.y - 1.5),
                    Vec2::splat(8.0),
                ),
                CornerRadius::ZERO,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.rect_stroke(
                Rect::from_center_size(
                    egui::pos2(centre.x - 1.5, centre.y + 1.5),
                    Vec2::splat(8.0),
                ),
                CornerRadius::ZERO,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        WindowButton::Close => {
            painter.line_segment(
                [
                    egui::pos2(centre.x - 5.0, centre.y - 5.0),
                    egui::pos2(centre.x + 5.0, centre.y + 5.0),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    egui::pos2(centre.x + 5.0, centre.y - 5.0),
                    egui::pos2(centre.x - 5.0, centre.y + 5.0),
                ],
                stroke,
            );
        }
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
