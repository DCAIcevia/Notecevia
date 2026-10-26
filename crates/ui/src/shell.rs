#![forbid(unsafe_code)]

//! 外壳：顶栏、左侧栏、两侧可收起的面板，把各块摆到各自的位置上。
//!
//! 这里只管位置和外观，不碰数据。要改数据的事都装进 `ShellOutput` 交回调用方。

use egui::{Align2, CornerRadius, FontId, Rect, Sense, Stroke, Ui, UiBuilder, Vec2};
use notecevia_core::BlockKind;

use crate::board::BoardApp;
use crate::home::{HomeAction, HomeApp};
use crate::icons::{self, Icon};
use crate::strings;
use crate::theme;

/// 顶栏高。外壳里所有面板的头部都跟它等高，横线才对得齐。
pub const TOP_BAR_HEIGHT: f32 = 48.0;
/// 全局工具栏高。横贯整窗，放在菜单栏下面。
pub const TOOLBAR_HEIGHT: f32 = 44.0;
/// 最左图标条宽。
pub const RAIL_WIDTH: f32 = 52.0;
/// 左侧边栏宽。
pub const LIST_WIDTH: f32 = 264.0;
/// 右侧资源栏宽。
pub const ASSETS_WIDTH: f32 = 280.0;
/// 图标条里按钮的边长与圆角。
const RAIL_BUTTON: f32 = 36.0;
/// 图标在按钮里留多少边：36 的按钮画 20 的图标，纯图标的栏不能再小。
const RAIL_ICON_PAD: f32 = 8.0;
const RAIL_RADIUS: u8 = 8;
/// 顶栏里搜索框的宽。
/// 资源栏里一行的行高。
const ASSET_ROW: f32 = 30.0;

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

/// 左侧栏的三层视图：一篇笔记 / 一组笔记 / 所有组。
///
/// 现在只有「页」有数据；「册」与「书架」是分类管理，等模型定了再接上。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    Page,
    Volume,
    Shelf,
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
    /// 菜单栏按出来的命令。
    pub menu: Option<MenuCommand>,
}

/// 外壳自己的界面状态：搜索词与两块面板的开合。与笔记数据无关。
pub struct Shell {
    query: String,
    list_open: bool,
    assets_open: bool,
    view: ViewMode,
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

impl Shell {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            list_open: true,
            assets_open: true,
            view: ViewMode::Page,
        }
    }

    pub fn ui(
        &mut self,
        ui: &mut Ui,
        home: &mut HomeApp,
        mut board: Option<&mut BoardApp>,
    ) -> ShellOutput {
        let mut output = ShellOutput::default();
        let full = ui.max_rect();
        let painter = ui.painter().clone();
        painter.rect_filled(full, CornerRadius::ZERO, theme::chrome().window);

        let top = Rect::from_min_size(full.min, Vec2::new(full.width(), TOP_BAR_HEIGHT));
        self.top_bar(ui, top, board.is_some(), &mut output);
        // 视图菜单只翻外壳自己的开关，不用惊动 app。
        match output.menu {
            Some(MenuCommand::ToggleList) => {
                self.list_open = !self.list_open;
                output.menu = None;
            }
            Some(MenuCommand::ToggleAssets) => {
                self.assets_open = !self.assets_open;
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
                false,
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
            self.rail_button(ui, rect, icon, label, self.view == mode, ready);
        }
        // 底部的两个收起键：纯图标栏的惯例位置，不占顶栏，也不跟视图混在一起。
        for (index, (icon, tip, open)) in [
            (Icon::PanelLeft, strings::SHELL_TOGGLE_LIST, self.list_open),
            (
                Icon::PanelRight,
                strings::SHELL_TOGGLE_ASSETS,
                self.assets_open,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let rect = Rect::from_center_size(
                egui::pos2(
                    rail.center().x,
                    rail.max.y
                        - theme::ui().gap_m
                        - RAIL_BUTTON / 2.0
                        - index as f32 * (RAIL_BUTTON + 2.0),
                ),
                Vec2::splat(RAIL_BUTTON),
            );
            if self.rail_toggle(ui, rect, icon, tip) {
                if index == 0 {
                    self.list_open = !open;
                } else {
                    self.assets_open = !open;
                }
            }
        }

        // 左侧边栏：笔记列表，由 HomeApp 画内容。
        let mut main_left = rail.max.x;
        if self.list_open {
            let list = Rect::from_min_size(
                egui::pos2(rail.max.x, body.min.y),
                Vec2::new(LIST_WIDTH, body.height()),
            );
            painter.rect_filled(list, CornerRadius::ZERO, theme::chrome().panel);
            painter.line_segment(
                [list.right_top(), list.right_bottom()],
                Stroke::new(1.0, theme::chrome().line),
            );
            ui.scope_builder(UiBuilder::new().max_rect(list), |ui| {
                output.home = home.sidebar(ui, &mut self.query);
            });
            main_left = list.max.x;
        }

        // 右侧资源栏：本轮只列名字与大小。
        let assets = board
            .as_ref()
            .map(|board| board.assets())
            .unwrap_or_default();
        let mut main_right = body.max.x;
        if self.assets_open {
            let panel = Rect::from_min_size(
                egui::pos2(body.max.x - ASSETS_WIDTH, body.min.y),
                Vec2::new(ASSETS_WIDTH, body.height()),
            );
            painter.rect_filled(panel, CornerRadius::ZERO, theme::chrome().panel);
            painter.line_segment(
                [panel.left_top(), panel.left_bottom()],
                Stroke::new(1.0, theme::chrome().line),
            );
            self.assets_panel(ui, panel, &assets);
            main_right = panel.min.x;
        }

        // 主区：有笔记就画笔记，没有就给起始态。
        let main = Rect::from_min_max(
            egui::pos2(main_left, body.min.y),
            egui::pos2(main_right, body.max.y),
        );
        if let Some(board) = board.as_mut() {
            // 工具栏属于画布：对整块画布全局，不当标签栏，也不横跨侧栏。
            let toolbar = Rect::from_min_size(main.min, Vec2::new(main.width(), TOOLBAR_HEIGHT));
            let notice = board.notice().map(|(text, error)| (text.to_owned(), error));
            self.toolbar(ui, toolbar, notice, Some(&mut *board));
            let canvas = Rect::from_min_max(egui::pos2(main.min.x, toolbar.max.y), main.max);
            ui.scope_builder(UiBuilder::new().max_rect(canvas), |ui| board.ui(ui));
        } else {
            self.start_state(ui, main, &mut output);
        }
        output
    }

    /// 顶栏：左折叠与菜单，右搜索与折叠资源栏。
    fn top_bar(&mut self, ui: &mut Ui, rect: Rect, has_note: bool, output: &mut ShellOutput) {
        // 顶栏只剩菜单：搜索搬进了左侧边栏，收起键搬到了图标条底部。
        let menus = Rect::from_min_max(
            egui::pos2(rect.min.x + theme::ui().gap_s, rect.min.y + 4.0),
            egui::pos2(rect.max.x - theme::ui().gap_m, rect.max.y - 4.0),
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
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, theme::chrome().line_soft),
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
                    if menu_item(ui, strings::MENU_HIDE_LIST, "", true) {
                        output.menu = Some(MenuCommand::ToggleList);
                    }
                    if menu_item(ui, strings::MENU_HIDE_ASSETS, "", true) {
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

    /// 资源栏：每行一个图标、一个名字、一个大小。没有资源时给一个拖入区。
    fn assets_panel(&self, ui: &mut Ui, rect: Rect, assets: &[crate::board::AssetRow]) {
        let painter = ui.painter().clone();
        let mut y = rect.min.y + TOP_BAR_HEIGHT;
        if assets.is_empty() {
            let zone = Rect::from_min_size(
                egui::pos2(rect.min.x + theme::ui().gap_m, y + theme::ui().gap_m),
                Vec2::new(rect.width() - theme::ui().gap_m * 2.0, 96.0),
            );
            painter.rect_filled(zone, CornerRadius::same(8), theme::chrome().field);
            painter.text(
                zone.center(),
                Align2::CENTER_CENTER,
                strings::ASSETS_DROP,
                FontId::proportional(theme::ui().meta),
                theme::chrome().text_faint,
            );
            return;
        }
        for asset in assets {
            let row = Rect::from_min_size(
                egui::pos2(rect.min.x, y),
                Vec2::new(rect.width(), ASSET_ROW),
            );
            let icon = match asset.kind {
                BlockKind::Image => Icon::Image,
                BlockKind::Audio => Icon::Audio,
                _ => Icon::Play,
            };
            icons::draw(
                &painter,
                Rect::from_center_size(
                    egui::pos2(row.min.x + theme::ui().gap_m + 7.0, row.center().y),
                    Vec2::splat(14.0),
                ),
                icon,
                theme::chrome().text_faint,
            );
            painter.text(
                egui::pos2(row.min.x + 32.0, row.center().y),
                Align2::LEFT_CENTER,
                &asset.name,
                FontId::proportional(theme::ui().meta + 0.5),
                theme::chrome().text_muted,
            );
            // 量不出大小时留白，不画 "0 B"：不知道就说不知道。
            if asset.bytes > 0 {
                painter.text(
                    egui::pos2(row.max.x - theme::ui().gap_m, row.center().y),
                    Align2::RIGHT_CENTER,
                    size_text(asset.bytes),
                    FontId::proportional(theme::ui().meta),
                    theme::chrome().text_faint,
                );
            }
            y += ASSET_ROW;
        }
    }

    /// 没有打开的笔记时，主区只给一件事：新建。
    fn start_state(&self, ui: &mut Ui, rect: Rect, output: &mut ShellOutput) {
        let painter = ui.painter().clone();
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
    ) {
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
        let _ = response.on_hover_text(tip);
    }

    /// 图标条底部的收起键：悬停出底，图标比视图键浅一档。
    fn rail_toggle(&self, ui: &mut Ui, rect: Rect, icon: Icon, label: &str) -> bool {
        let response = ui.interact(rect, ui.scope_id().with(label), Sense::click());
        let painter = ui.painter();
        if response.hovered() {
            painter.rect_filled(rect, CornerRadius::same(RAIL_RADIUS), theme::chrome().hover);
        }
        let color = if response.hovered() {
            theme::chrome().text
        } else {
            theme::chrome().text_faint
        };
        icons::draw(painter, rect.shrink(RAIL_ICON_PAD), icon, color);
        response.on_hover_text(label).clicked()
    }
}

/// 文件大小只留一位小数，够看就行。
fn size_text(bytes: u64) -> String {
    const K: f64 = 1024.0;
    let value = bytes as f64;
    if value >= K * K {
        format!("{:.1} MB", value / (K * K))
    } else if value >= K {
        format!("{:.0} KB", value / K)
    } else {
        format!("{bytes} B")
    }
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
