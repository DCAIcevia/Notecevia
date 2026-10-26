#![forbid(unsafe_code)]

//! 册视图：左边是册列表，右边是这本册的目录。
//!
//! 这里只画，不碰数据。改名、删除、移出页这些动作都经 `LibraryAction` 交回 app。

use egui::{Align2, CornerRadius, FontId, Rect, Sense, Stroke, Ui, Vec2};

use crate::icons::{self, Icon};
use crate::strings;
use crate::theme;

/// 册列表里的一行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeRow {
    pub id: u64,
    pub title: String,
    pub pages: usize,
}

/// 目录里的一行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageRow {
    pub id: u64,
    pub title: String,
}

/// 册视图按出来的动作。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryAction {
    /// 新建一本册。
    New,
    /// 打开一本册。
    Open(u64),
    /// 改名。
    Rename(u64, String),
    /// 删掉这本册（连同页一起）。
    Delete(u64),
    /// 在目录里点开一页。
    OpenPage(u64),
    /// 把一页从这本册里移出（回到 Empty book）。
    RemoveFromVolume(u64),
}

/// 册视图的界面状态。
pub struct Library {
    volumes: Vec<VolumeRow>,
    pages: Vec<PageRow>,
    current: Option<u64>,
    empty_book: u64,
    pending_delete: Option<u64>,
}

impl Library {
    pub fn new(empty_book: u64) -> Self {
        Self {
            volumes: Vec::new(),
            pages: Vec::new(),
            current: None,
            empty_book,
            pending_delete: None,
        }
    }

    pub fn set_volumes(&mut self, volumes: Vec<VolumeRow>) {
        self.volumes = volumes;
    }

    pub fn set_pages(&mut self, pages: Vec<PageRow>) {
        self.pages = pages;
    }

    pub fn set_current(&mut self, id: Option<u64>) {
        self.current = id;
    }

    pub fn volumes(&self) -> &[VolumeRow] {
        &self.volumes
    }

    pub fn current(&self) -> Option<u64> {
        self.current
    }

    /// 左侧：册列表。顶部一行只有右边一个「＋」。
    pub fn sidebar(&mut self, ui: &mut Ui) -> Option<LibraryAction> {
        let mut action = None;
        let (_, row) = ui.allocate_space(Vec2::new(ui.available_width(), 48.0));
        let plus = Rect::from_center_size(
            egui::pos2(
                row.right() - theme::ui().gap_m - theme::ui().control_height / 2.0,
                row.center().y,
            ),
            Vec2::splat(theme::ui().control_height),
        );
        if icon_button(ui, plus, Icon::Plus, strings::LIBRARY_NEW) {
            action = Some(LibraryAction::New);
        }

        let mut y = row.bottom();
        for volume in &self.volumes {
            let rect = Rect::from_min_size(
                egui::pos2(ui.max_rect().left(), y),
                Vec2::new(ui.available_width(), 52.0),
            );
            let response = ui.interact(
                rect,
                ui.scope_id().with(("volume-row", volume.id)),
                Sense::click(),
            );
            let selected = Some(volume.id) == self.current;
            if selected {
                ui.painter().rect_filled(
                    rect.shrink2(Vec2::new(0.0, 0.0)),
                    CornerRadius::same(theme::ui().radius_control),
                    theme::chrome().selected,
                );
            } else if response.hovered() {
                ui.painter().rect_filled(
                    rect,
                    CornerRadius::same(theme::ui().radius_control),
                    theme::chrome().hover,
                );
            }
            label(
                ui,
                egui::pos2(rect.left() + theme::ui().gap_m, rect.center().y - 9.0),
                &volume.title,
                theme::ui().body,
                theme::chrome().text,
                true,
            );
            label(
                ui,
                egui::pos2(rect.left() + theme::ui().gap_m, rect.center().y + 9.0),
                &strings::page_count(volume.pages),
                theme::ui().meta,
                theme::chrome().text_faint,
                false,
            );
            if response.clicked() {
                action = Some(LibraryAction::Open(volume.id));
            }
            if volume.id != self.empty_book {
                response.context_menu(|ui| {
                    if ui.button(strings::LIBRARY_DELETE).clicked() {
                        self.pending_delete = Some(volume.id);
                        ui.close();
                    }
                });
            }
            y += 52.0;
        }

        // 删册是连带删页的，问一句再动手。
        if let Some(id) = self.pending_delete {
            let title = self
                .volumes
                .iter()
                .find(|volume| volume.id == id)
                .map(|volume| volume.title.clone())
                .unwrap_or_default();
            let mut confirmed = false;
            let mut cancelled = false;
            egui::Modal::new(ui.scope_id().with("delete-volume")).show(ui.ctx(), |ui| {
                ui.label(strings::LIBRARY_DELETE_TITLE);
                ui.label(format!("《{title}》"));
                ui.add_space(theme::ui().gap_m);
                ui.label(strings::LIBRARY_DELETE_NOTE);
                ui.add_space(theme::ui().gap_m);
                ui.horizontal(|ui| {
                    if ui.button(strings::LIBRARY_DELETE_CONFIRM).clicked() {
                        confirmed = true;
                    }
                    if ui.button(strings::LIBRARY_CANCEL).clicked() {
                        cancelled = true;
                    }
                });
            });
            if confirmed {
                action = Some(LibraryAction::Delete(id));
                self.pending_delete = None;
            } else if cancelled {
                self.pending_delete = None;
            }
        }
        action
    }

    /// 主区：这本册的目录。
    pub fn pages_view(&mut self, ui: &mut Ui) -> Option<LibraryAction> {
        let mut action = None;
        let rect = ui.max_rect();
        let painter = ui.painter().clone();
        painter.rect_filled(rect, CornerRadius::ZERO, theme::chrome().window);

        let current = self.current?;
        let title = self
            .volumes
            .iter()
            .find(|volume| volume.id == current)
            .map(|volume| volume.title.clone())
            .unwrap_or_default();

        let bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 44.0));
        painter.line_segment(
            [bar.left_bottom(), bar.right_bottom()],
            Stroke::new(1.0, theme::chrome().line_soft),
        );
        label(
            ui,
            egui::pos2(bar.left() + theme::ui().gap_l, bar.center().y),
            &title,
            theme::ui().title,
            theme::chrome().text,
            false,
        );

        if self.pages.is_empty() {
            let centre = egui::pos2(rect.center().x, rect.min.y + rect.height() * 0.36);
            painter.text(
                centre,
                Align2::CENTER_CENTER,
                strings::LIBRARY_EMPTY,
                FontId::proportional(theme::ui().empty),
                theme::chrome().text_muted,
            );
            return None;
        }

        let mut y = bar.bottom();
        for (index, page) in self.pages.iter().enumerate() {
            let row =
                Rect::from_min_size(egui::pos2(rect.left(), y), Vec2::new(rect.width(), 44.0));
            let response = ui.interact(
                row,
                ui.scope_id().with(("toc-row", page.id)),
                Sense::click(),
            );
            if response.hovered() {
                painter.rect_filled(
                    row,
                    CornerRadius::same(theme::ui().radius_control),
                    theme::chrome().hover,
                );
            }
            label(
                ui,
                egui::pos2(row.left() + theme::ui().gap_l, row.center().y),
                &page.title,
                theme::ui().body,
                theme::chrome().text,
                false,
            );
            painter.text(
                egui::pos2(row.right() - theme::ui().gap_l - 24.0, row.center().y),
                Align2::RIGHT_CENTER,
                format!("{}", index + 1),
                FontId::monospace(theme::ui().meta),
                theme::chrome().text_faint,
            );
            if response.clicked() {
                action = Some(LibraryAction::OpenPage(page.id));
            }
            response.context_menu(|ui| {
                if ui.button(strings::LIBRARY_REMOVE).clicked() {
                    action = Some(LibraryAction::RemoveFromVolume(page.id));
                    ui.close();
                }
            });
            y += 44.0;
        }
        action
    }
}

fn label(ui: &Ui, pos: egui::Pos2, text: &str, size: f32, colour: egui::Color32, _strong: bool) {
    ui.painter().text(
        pos,
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(size),
        colour,
    );
}

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
    let colour = if hovered {
        theme::chrome().text
    } else {
        theme::chrome().text_muted
    };
    icons::draw(ui.painter(), rect.shrink(6.0), icon, colour);
    response.on_hover_text(label).clicked()
}
