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
use crate::window::{CaptionPointer, CaptionZones, WindowHooks};

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
    /// 正在改窗口大小：按下时的外框、指针屏幕坐标，以及从哪条边改。
    window_resize: Option<(Rect, (f32, f32), egui::ResizeDirection)>,
    /// 标题。系统标题栏没了，文档名只剩这一处能看见。
    title: String,
    /// 钉住的焦点状态。截图时用：进程有没有前台是随机的，不钉住同一份代码会画出两张图。
    focus_pinned: Option<bool>,
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
            window_resize: None,
            title: String::new(),
            focus_pinned: None,
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

    /// 把焦点状态钉住。截图验收用：真实焦点取决于进程有没有前台，随机。
    pub fn pin_focus(&mut self, focused: bool) {
        self.focus_pinned = Some(focused);
    }

    /// 现在算不算有焦点。钉住了就听钉的值。
    fn window_focused(&self, ui: &Ui) -> bool {
        self.focus_pinned.unwrap_or_else(|| {
            ui.ctx()
                .input(|input| input.viewport().focused.unwrap_or(true))
        })
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
        // 窗口圆角由 DWM 裁，这里画满整块，不要自己用透明去抠。
        painter.rect_filled(full, CornerRadius::ZERO, theme::chrome().ground);

        let header = Rect::from_min_max(
            full.min + Vec2::splat(ISLAND_MARGIN),
            egui::pos2(
                full.max.x - ISLAND_MARGIN,
                full.min.y + ISLAND_MARGIN + HEADER_HEIGHT,
            ),
        );
        self.resize_edges(ui, full, hooks);
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
        let strip = drag_area(rect, minimize.min.x);
        let drag = ui.interact(
            strip,
            ui.scope_id().with("shell-drag"),
            Sense::click_and_drag(),
        );
        // 拖动交给系统：窗口整个一起搬，没有内容要重画，所以不存在帧间错位，
        // 手指到哪窗口到哪；顺带把贴边吸附也拿回来。自己逐帧发位置会落后一帧，
        // 手感是"追着指针跑"。
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        // 微软的标题栏规范：没有交互元素的地方右键出系统窗口菜单。
        if drag.secondary_clicked()
            && let Some((x, y)) = cursor_points(ui, hooks)
        {
            hooks.show_system_menu(x, y);
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
            let colour = if self.window_focused(ui) {
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

        // 图标与最大化按钮交给系统：贴靠浮窗和系统菜单只认系统自己认得的区域。
        // 报的是物理像素——系统那边没有"逻辑点"这回事。
        let scale = ui.ctx().pixels_per_point();
        let physical = |r: Rect| {
            Rect::from_min_max(
                egui::pos2(r.min.x * scale, r.min.y * scale),
                egui::pos2(r.max.x * scale, r.max.y * scale),
            )
        };
        hooks.caption_zones(CaptionZones {
            icon: Some(physical(mark)),
            maximize: Some(physical(maximize)),
        });

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
    ///
    /// 最大化按钮在系统接管标题栏时不归界面管：贴靠浮窗只在系统认得的按钮区域
    /// 里出现，而那块地方界面已经收不到指针事件。所以这里不登记控件、不抢点击，
    /// 悬停与按下问系统要——画出来的样子仍和其他两个按钮一模一样。
    fn window_button(&self, ui: &mut Ui, rect: Rect, kind: WindowButton, hooks: &dyn WindowHooks) {
        let chrome = theme::chrome();
        let closing = kind == WindowButton::Close;
        let focused = self.window_focused(ui);
        let system = matches!(kind, WindowButton::Maximize | WindowButton::Restore)
            && hooks.handles_caption();
        let response = (!system).then(|| {
            ui.interact(
                rect,
                ui.scope_id().with(("window-button", kind as u8)),
                Sense::click(),
            )
        });
        let pointer = match (&response, system) {
            (_, true) => hooks.caption_pointer().unwrap_or_default(),
            (Some(response), false) => CaptionPointer {
                hovered: response.hovered(),
                pressed: response.is_pointer_button_down_on(),
            },
            (None, false) => CaptionPointer::default(),
        };
        // 悬停、按下、静置三档底色都取主题；系统标题栏按钮是整格出血的，
        // 这里只内缩 2——它坐在岛里，出血到边缘会把卡的圆角盖成方的。
        let fill = if pointer.pressed {
            Some(if closing {
                chrome.danger
            } else {
                chrome.pressed
            })
        } else if pointer.hovered {
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
        let colour = if pointer.hovered && closing {
            chrome.window
        } else if !focused {
            chrome.text_faint
        } else if pointer.hovered {
            chrome.text
        } else {
            chrome.text_muted
        };
        paint_window_glyph(ui.painter(), rect, kind, colour);
        let Some(response) = response else {
            return;
        };
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
    fn resize_edges(&mut self, ui: &mut Ui, full: Rect, hooks: &dyn WindowHooks) {
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
            if !response.drag_started() {
                continue;
            }
            // 拿不到系统指针，说明这平台没有窗口翻译层（Linux、macOS）：退回系统的
            // 缩放通道。它有自己的模态循环，画面会滞后，但比拉不动强，也不用为
            // 每个平台写一遍取指针的代码。
            let Some(start) = cursor_points(ui, hooks) else {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
                continue;
            };
            // 自己算尺寸，逐帧发命令：系统的 DRAG_RESIZE 是一个模态循环，
            // 那段时间应用完全不出帧，你看到的是旧画面被系统拉着走——
            // 正是"鼠标过去了内容还在原地、然后猛地跳一下"那种残影。
            if let Some(outer) = ui.ctx().input(|input| input.viewport().outer_rect) {
                self.window_resize = Some((outer, start, direction));
            }
        }
        let Some((start, grab, direction)) = self.window_resize else {
            return;
        };
        // 松手判定问系统：窗口在拖动中被搬动时，抬起事件可能到不了应用，
        // 只看应用记的状态会让"正在拖"永远清不掉——表现就是松手之后尺寸还跟着鼠标走。
        let down = hooks
            .primary_button_down()
            .unwrap_or_else(|| ui.ctx().input(|input| input.pointer.any_down()));
        if !down {
            self.window_resize = None;
            return;
        }
        // 指针位置也问系统要：窗口自己在动，用局部坐标算会形成回路。
        if let Some(now) = cursor_points(ui, hooks) {
            let delta = Vec2::new(now.0 - grab.0, now.1 - grab.1);
            let rect = resized(start, delta, direction);
            // 位置只在真的会变的时候才发：拉右边、下边时位置不动，发了只会让窗口
            // 白跑一次 set_outer_position——那一步最容易把鼠标捕获搞掉。
            if rect.min != start.min {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::OuterPosition(rect.min));
            }
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(rect.size()));
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

/// 指针在屏幕上的位置，按界面点数算。
///
/// 问系统要，不拿窗口局部坐标换算：窗口自己在动，局部坐标会跟着变，
/// 用它算位移就成了回路，尺寸能一路滚到把绘制后端撑崩。
fn cursor_points(ui: &Ui, hooks: &dyn WindowHooks) -> Option<(f32, f32)> {
    let (x, y) = hooks.cursor_position()?;
    let scale = ui.ctx().pixels_per_point();
    Some((x / scale, y / scale))
}

/// 标题栏上能拖窗口的那块：左边从头部起点，右边到三颗按钮为止。
///
/// 抽成函数是为了测得着。这里曾经写成 from_min_max(rect.min, 按钮的 min)，
/// 而按钮的 min.y 正好等于头部顶边，于是拖区是一条零高度的线，永远命中不了——
/// 三次"拖不动"都栽在这儿，却一直去改下游。
fn drag_area(header: Rect, buttons_left: f32) -> Rect {
    Rect::from_min_max(header.min, egui::pos2(buttons_left, header.max.y))
}

/// 窗口最小尺寸。比这个再小就没法用了。
const MIN_WINDOW_WIDTH: f32 = 720.0;
const MIN_WINDOW_HEIGHT: f32 = 420.0;

/// 按拖了多少算出新的窗口外框。动哪条边由方向决定，另一边保持不动。
fn resized(start: Rect, delta: Vec2, direction: egui::ResizeDirection) -> Rect {
    use egui::ResizeDirection as Direction;
    let moves_left = matches!(
        direction,
        Direction::West | Direction::NorthWest | Direction::SouthWest
    );
    let moves_top = matches!(
        direction,
        Direction::North | Direction::NorthWest | Direction::NorthEast
    );
    let moves_right = matches!(
        direction,
        Direction::East | Direction::NorthEast | Direction::SouthEast
    );
    let moves_bottom = matches!(
        direction,
        Direction::South | Direction::SouthWest | Direction::SouthEast
    );
    let mut min = start.min;
    let mut max = start.max;
    if moves_left {
        min.x += delta.x;
    }
    if moves_right {
        max.x += delta.x;
    }
    if moves_top {
        min.y += delta.y;
    }
    if moves_bottom {
        max.y += delta.y;
    }
    // 最小尺寸由这里兜：动的那条边推回去，对面那条边不动。
    let width = (max.x - min.x).max(MIN_WINDOW_WIDTH);
    let height = (max.y - min.y).max(MIN_WINDOW_HEIGHT);
    let left = if moves_left { max.x - width } else { min.x };
    let top = if moves_top { max.y - height } else { min.y };
    Rect::from_min_size(egui::pos2(left, top), Vec2::new(width, height))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    /// 测试用的窗口：指针在哪、按键按没按、菜单有没有被叫出来，全由测试给定。
    /// 真实窗口不可测，但窗口在接口后面，就可以从这里替换掉。
    struct FakeWindow {
        cursor: Cell<(f32, f32)>,
        down: Cell<bool>,
        maximized: Cell<bool>,
        menus: Cell<u32>,
        /// 有没有窗口翻译层。false 就是 Linux、macOS：拿不到系统指针。
        platform: Cell<bool>,
        /// 系统有没有接管图标与最大化按钮。
        caption: Cell<bool>,
        /// 系统上报的那一块的悬停与按下。
        pointer: Cell<CaptionPointer>,
        /// 界面报上来的两块区域。
        zones: Cell<Option<CaptionZones>>,
    }

    impl Default for FakeWindow {
        fn default() -> Self {
            Self {
                cursor: Cell::new((0.0, 0.0)),
                down: Cell::new(false),
                maximized: Cell::new(false),
                menus: Cell::new(0),
                platform: Cell::new(true),
                caption: Cell::new(false),
                pointer: Cell::new(CaptionPointer::default()),
                zones: Cell::new(None),
            }
        }
    }

    impl WindowHooks for FakeWindow {
        fn show_system_menu(&self, _x: f32, _y: f32) -> bool {
            self.menus.set(self.menus.get() + 1);
            true
        }

        fn primary_button_down(&self) -> Option<bool> {
            Some(self.down.get())
        }

        fn cursor_position(&self) -> Option<(f32, f32)> {
            self.platform.get().then(|| self.cursor.get())
        }

        fn toggle_maximize(&self) -> Option<bool> {
            self.maximized.set(!self.maximized.get());
            Some(self.maximized.get())
        }

        fn is_maximized(&self) -> Option<bool> {
            Some(self.maximized.get())
        }

        fn caption_zones(&self, zones: CaptionZones) {
            self.zones.set(Some(zones));
        }

        fn handles_caption(&self) -> bool {
            self.caption.get()
        }

        fn caption_pointer(&self) -> Option<CaptionPointer> {
            self.caption.get().then(|| self.pointer.get())
        }
    }

    const SCREEN: Vec2 = Vec2::new(800.0, 600.0);
    /// 窗口在屏幕上的左上角。外壳拿它和指针坐标一起算位移。
    const WINDOW_AT: egui::Pos2 = egui::Pos2::new(100.0, 100.0);

    fn click(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }

    /// 跑一帧外壳，把这一帧发给窗口的视口命令捞回来。
    fn frame(
        ctx: &egui::Context,
        shell: &mut Shell,
        window: &FakeWindow,
        time: f64,
        events: Vec<egui::Event>,
    ) -> Vec<egui::ViewportCommand> {
        let mut input = egui::RawInput {
            time: Some(time),
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
            events,
            ..Default::default()
        };
        input.viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                outer_rect: Some(Rect::from_min_size(WINDOW_AT, SCREEN)),
                inner_rect: Some(Rect::from_min_size(WINDOW_AT, SCREEN)),
                focused: Some(true),
                ..Default::default()
            },
        );
        let mut home = HomeApp::new();
        let mut trash = TrashView::new();
        let mut media = MediaPanel::new();
        let mut output = ctx.run_ui(input, |ui| {
            let views = Views {
                home: &mut home,
                library: None,
                trash: &mut trash,
                media_panel: &mut media,
                board: None,
            };
            shell.ui(ui, window, views);
        });
        output.textures_delta.clear();
        output
            .viewport_output
            .remove(&egui::ViewportId::ROOT)
            .map(|viewport| viewport.commands)
            .unwrap_or_default()
    }

    fn resizes(commands: &[egui::ViewportCommand]) -> bool {
        commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::InnerSize(_)))
    }

    #[test]
    fn drag_area_has_area_and_stops_before_the_buttons() {
        let header = Rect::from_min_size(egui::pos2(6.0, 6.0), Vec2::new(1268.0, 32.0));
        let area = drag_area(header, 1154.0);
        // 曾经这里高度是 0：一条线永远命中不了。三次拖不动的根都在这一行。
        assert!(area.height() > 0.0, "拖区必须有高度，否则永远点不到");
        assert!(area.width() > 0.0, "拖区必须有宽度");
        assert_eq!(area.min, header.min, "拖区从头部左边开始");
        assert!(area.max.x <= 1154.0, "拖区不能压到窗口按钮上");
        assert!(header.contains_rect(area), "拖区必须在头部里面");
    }

    #[test]
    fn resizing_keeps_the_edge_that_did_not_move() {
        use egui::ResizeDirection as Direction;
        let start = Rect::from_min_size(egui::pos2(100.0, 100.0), Vec2::new(800.0, 600.0));
        // 没动就不该变：任何方向、零位移都必须原样返回。
        for direction in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
            Direction::NorthEast,
            Direction::SouthEast,
            Direction::NorthWest,
            Direction::SouthWest,
        ] {
            assert_eq!(resized(start, Vec2::ZERO, direction), start);
        }
        // 拉右边：左边不许动。
        let east = resized(start, Vec2::new(40.0, 0.0), Direction::East);
        assert_eq!(east.min, start.min);
        assert_eq!(east.width(), 840.0);
        // 拉左边：右边不许动。
        let west = resized(start, Vec2::new(40.0, 0.0), Direction::West);
        assert_eq!(west.max.x, start.max.x);
        assert_eq!(west.width(), 760.0);
        // 拉左下角：左边和下边一起动，所以右边与上边才是不动的。
        let sw = resized(start, Vec2::new(-30.0, 20.0), Direction::SouthWest);
        assert_eq!(sw.max.x, start.max.x, "左下角只动左、下两条边");
        assert_eq!(sw.min.y, start.min.y, "上边不该跟着动");
        assert_eq!(sw.max.y, 720.0, "下边往下走了 20");
        assert_eq!(sw.width(), 830.0);
        assert_eq!(sw.height(), 620.0);
    }

    #[test]
    fn caption_zones_cover_the_icon_and_the_maximize_button() {
        // 交给系统的是两块具体矩形，不是"某处"。系统按像素命中，差一格就等于没交。
        let ctx = egui::Context::default();
        let mut shell = Shell::new();
        let window = FakeWindow::default();
        frame(&ctx, &mut shell, &window, 0.0, Vec::new());
        let zones = window.zones.get().expect("每帧都得把交给系统的两块报上去");
        let icon = zones.icon.expect("图标交给系统，才有单击菜单与双击关窗");
        let maximize = zones.maximize.expect("最大化交给系统，才有贴靠浮窗");
        assert_eq!(icon.size(), Vec2::splat(MARK), "图标那一格是 16 见方");
        assert_eq!(icon.min.x, ISLAND_MARGIN + MARK_INSET, "图标离左边界 16");
        assert_eq!(maximize.width(), WINDOW_BUTTON, "最大化按钮 46 宽");
        assert_eq!(
            maximize.max.x,
            SCREEN.x - ISLAND_MARGIN - WINDOW_BUTTON,
            "最大化按钮在关闭按钮左边"
        );
        assert!(!icon.intersects(maximize), "两块重叠，图标会抢走贴靠浮窗");
    }

    #[test]
    fn caption_zones_are_reported_in_physical_pixels() {
        // 系统命中的是物理像素。高分屏上把逻辑点当物理像素报上去，
        // 命中区会缩到一半、落到别的地方。
        let ctx = egui::Context::default();
        let mut shell = Shell::new();
        let window = FakeWindow::default();
        frame(&ctx, &mut shell, &window, 0.0, Vec::new());
        let logical = window.zones.get().and_then(|zones| zones.icon).unwrap();
        ctx.set_pixels_per_point(2.0);
        frame(&ctx, &mut shell, &window, 0.10, Vec::new());
        let physical = window.zones.get().and_then(|zones| zones.icon).unwrap();
        assert_eq!(physical.min.x, logical.min.x * 2.0);
        assert_eq!(physical.width(), logical.width() * 2.0);
    }

    #[test]
    fn maximize_button_is_left_to_the_system() {
        // 系统接管之后点击由系统处理。界面要是也点一次，一下等于切两下：
        // 最大化了立刻又还原，看着像"点不动"。
        let ctx = egui::Context::default();
        let mut shell = Shell::new();
        let window = FakeWindow::default();
        window.caption.set(true);
        frame(&ctx, &mut shell, &window, 0.0, Vec::new());
        let at = window
            .zones
            .get()
            .and_then(|zones| zones.maximize)
            .expect("最大化按钮得报上去")
            .center();
        frame(
            &ctx,
            &mut shell,
            &window,
            0.10,
            vec![egui::Event::PointerMoved(at), click(at, true)],
        );
        let commands = frame(&ctx, &mut shell, &window, 0.20, vec![click(at, false)]);
        assert!(!window.maximized.get(), "系统接管时界面不许再切一次");
        assert!(
            !commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::Maximized(_))),
            "系统接管时界面不许再发最大化命令"
        );

        // 没有系统接管（Linux、macOS）：界面自己切，这条不能丢。
        window.caption.set(false);
        // 先空跑一帧让按钮登记成控件，再点：egui 的点击按上一帧的控件表命中。
        frame(&ctx, &mut shell, &window, 0.30, Vec::new());
        frame(
            &ctx,
            &mut shell,
            &window,
            0.40,
            vec![egui::Event::PointerMoved(at), click(at, true)],
        );
        frame(&ctx, &mut shell, &window, 0.50, vec![click(at, false)]);
        assert!(window.maximized.get(), "没有系统接管时界面得自己切最大化");
    }

    #[test]
    fn resizing_falls_back_to_the_system_when_no_platform_layer() {
        // Linux、macOS 没有窗口翻译层，拿不到系统指针。这条回退不成立，
        // 那些平台上窗口根本拉不动——所以它必须有测试守着。
        let ctx = egui::Context::default();
        let mut shell = Shell::new();
        let window = FakeWindow::default();
        window.platform.set(false);
        let edge = egui::pos2(400.0, SCREEN.y - 4.0);
        frame(&ctx, &mut shell, &window, 0.0, Vec::new());
        window.down.set(true);
        frame(
            &ctx,
            &mut shell,
            &window,
            0.10,
            vec![egui::Event::PointerMoved(edge), click(edge, true)],
        );
        let commands = frame(
            &ctx,
            &mut shell,
            &window,
            0.20,
            vec![egui::Event::PointerMoved(edge - Vec2::new(0.0, 10.0))],
        );
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::BeginResize(_))),
            "没有平台层时要退回系统的缩放通道"
        );
        assert!(!resizes(&commands), "没有系统指针时界面算不出新尺寸");
    }

    #[test]
    fn resizing_never_goes_below_the_minimum() {
        use egui::ResizeDirection as Direction;
        let start = Rect::from_min_size(egui::pos2(100.0, 100.0), Vec2::new(800.0, 600.0));
        let tiny = resized(start, Vec2::new(-5000.0, -5000.0), Direction::SouthEast);
        assert_eq!(tiny.width(), MIN_WINDOW_WIDTH);
        assert_eq!(tiny.height(), MIN_WINDOW_HEIGHT);
        assert_eq!(tiny.min, start.min, "拉右下角时左上角不能漂移");
    }

    #[test]
    fn letting_go_stops_the_resize() {
        // 曾经松手之后窗口还跟着鼠标走：位置命令每帧都发，把鼠标捕获搞丢了，
        // 抬起事件到不了应用，状态就永远清不掉。这条测试盯的就是那一步。
        let ctx = egui::Context::default();
        let mut shell = Shell::new();
        let window = FakeWindow::default();
        window.down.set(true);
        // 下边缘那条带子：窗口顶在 (100,100)，底边在 700。
        window.cursor.set((400.0, 696.0));
        frame(&ctx, &mut shell, &window, 0.0, Vec::new());
        frame(
            &ctx,
            &mut shell,
            &window,
            0.10,
            vec![
                egui::Event::PointerMoved(egui::pos2(400.0, 596.0)),
                click(egui::pos2(400.0, 596.0), true),
            ],
        );
        window.cursor.set((400.0, 686.0));
        let dragging = frame(
            &ctx,
            &mut shell,
            &window,
            0.20,
            vec![egui::Event::PointerMoved(egui::pos2(400.0, 586.0))],
        );
        assert!(resizes(&dragging), "按住拖动时应该改尺寸");

        // 松手：系统说键已经抬起，状态必须当帧清掉。
        window.down.set(false);
        let released = frame(
            &ctx,
            &mut shell,
            &window,
            0.30,
            vec![click(egui::pos2(400.0, 586.0), false)],
        );
        assert!(!resizes(&released), "松手那一帧不该再改尺寸");

        // 之后只移动鼠标，尺寸一个字节都不许动。
        window.cursor.set((420.0, 300.0));
        let after = frame(
            &ctx,
            &mut shell,
            &window,
            0.40,
            vec![egui::Event::PointerMoved(egui::pos2(400.0, 400.0))],
        );
        assert!(!resizes(&after), "松手之后还跟着鼠标，就是那个 bug 回来了");
    }
}
