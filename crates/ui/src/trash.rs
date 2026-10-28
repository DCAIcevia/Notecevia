#![forbid(unsafe_code)]

//! 回收站视图：删掉的东西摆一列，能还原，也能真删。
//!
//! 这里只管画和收动作：名字、剩余天数、大小都由 app 算好递进来，界面不碰文件，
//! 也不读时钟。彻底删除不可逆，所以点两下才算数。

use egui::{Align2, Button, CornerRadius, FontId, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::icons::{self, Icon};
use crate::strings;
use crate::theme;

/// 一行的高度。
const ROW: f32 = 44.0;
/// 头部高度：标题、规矩、清空按钮都在这条里。
const HEAD: f32 = 56.0;
/// 行里一个动作按钮的宽。「彻底删除」是最长的那个，按它定。
const ACTION: f32 = 76.0;
/// 头部清空按钮的宽。
const EMPTY_ACTION: f32 = 88.0;

/// 回收站里一条东西的种类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrashKind {
    Board,
    Volume,
    Shelf,
}

impl TrashKind {
    fn icon(self) -> Icon {
        match self {
            Self::Board => Icon::Page,
            Self::Volume => Icon::Volume,
            Self::Shelf => Icon::Shelf,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Board => strings::TRASH_KIND_BOARD,
            Self::Volume => strings::TRASH_KIND_VOLUME,
            Self::Shelf => strings::TRASH_KIND_SHELF,
        }
    }
}

/// 一条要摆出来的东西。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashRow {
    /// 回收站里的条目名。动作按它指回是哪一条。
    pub folder: String,
    pub kind: TrashKind,
    /// 名字。空串时界面给占位。
    pub title: String,
    /// 还剩几天到期，0 表示今天到期。
    pub days_left: i64,
    pub bytes: u64,
}

/// 按出来的动作。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrashAction {
    Restore(String),
    Purge(String),
    Empty,
}

/// 回收站视图自己的界面状态。
pub struct TrashView {
    rows: Vec<TrashRow>,
    /// 保留期，天。由 app 递进来，别在这个界面里另写一个 30。
    keep_days: i64,
    /// 正在等第二次确认的那一条。
    confirming: Option<String>,
    /// 清空也要确认。
    confirm_empty: bool,
}

impl Default for TrashView {
    fn default() -> Self {
        Self::new()
    }
}

impl TrashView {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            keep_days: 0,
            confirming: None,
            confirm_empty: false,
        }
    }

    /// 换一批数据。正在确认的那条要是已经不在了，确认状态跟着作废。
    pub fn set_rows(&mut self, rows: Vec<TrashRow>) {
        if let Some(folder) = &self.confirming
            && !rows.iter().any(|row| &row.folder == folder)
        {
            self.confirming = None;
        }
        if rows.is_empty() {
            self.confirm_empty = false;
        }
        self.rows = rows;
    }

    /// 保留期，天。只在数据来得时候跟着 store 走，界面不自己猜。
    pub fn set_keep_days(&mut self, days: i64) {
        self.keep_days = days;
    }

    pub fn rows(&self) -> &[TrashRow] {
        &self.rows
    }

    /// 画一屏，返回按出来的动作。
    pub fn ui(&mut self, ui: &mut Ui, rect: Rect) -> Option<TrashAction> {
        let painter = ui.painter().clone();
        let chrome = theme::chrome();
        let tokens = theme::ui();
        let left = rect.min.x + tokens.gap_l;
        let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEAD));
        painter.text(
            egui::pos2(left, head.center().y - 9.0),
            Align2::LEFT_CENTER,
            strings::TRASH_TITLE,
            FontId::proportional(tokens.title),
            chrome.text,
        );
        painter.text(
            egui::pos2(left, head.center().y + 9.0),
            Align2::LEFT_CENTER,
            strings::trash_hint(self.keep_days),
            FontId::proportional(tokens.meta),
            chrome.text_faint,
        );

        let mut action = None;
        if !self.rows.is_empty() {
            painter.line_segment(
                [head.left_bottom(), head.right_bottom()],
                Stroke::new(1.0, chrome.line_soft),
            );
            let (label, colour) = if self.confirm_empty {
                (strings::TRASH_EMPTY_CONFIRM, chrome.danger)
            } else {
                (strings::TRASH_EMPTY_ACTION, chrome.text_muted)
            };
            let button = Rect::from_min_size(
                egui::pos2(
                    rect.max.x - tokens.gap_l - EMPTY_ACTION,
                    head.center().y - tokens.control_height / 2.0,
                ),
                Vec2::new(EMPTY_ACTION, tokens.control_height),
            );
            let response = ui.put(
                button,
                Button::new(RichText::new(label).size(tokens.meta).color(colour)).frame(false),
            );
            if response.clicked() {
                if self.confirm_empty {
                    action = Some(TrashAction::Empty);
                } else {
                    self.confirm_empty = true;
                }
            }
        }

        if self.rows.is_empty() {
            let anchor = egui::pos2(rect.center().x, rect.min.y + rect.height() * 0.34);
            painter.text(
                anchor,
                Align2::CENTER_CENTER,
                strings::TRASH_EMPTY_TITLE,
                FontId::proportional(tokens.empty),
                chrome.text_muted,
            );
            return action;
        }

        // 一列：点出来的动作先记着，循环里只读 self.rows。
        let mut restore: Option<String> = None;
        let mut purge: Option<String> = None;
        let mut confirm: Option<String> = None;
        let mut y = rect.min.y + HEAD;
        for row in &self.rows {
            let line = Rect::from_min_size(egui::pos2(rect.min.x, y), Vec2::new(rect.width(), ROW));
            let hovered = ui
                .interact(
                    line,
                    ui.scope_id().with(("trash-row", row.folder.as_str())),
                    Sense::hover(),
                )
                .hovered();
            if hovered {
                painter.rect_filled(line, CornerRadius::ZERO, chrome.hover);
            }
            painter.line_segment(
                [line.left_bottom(), line.right_bottom()],
                Stroke::new(1.0, chrome.line_soft),
            );
            icons::draw(
                &painter,
                Rect::from_center_size(egui::pos2(left + 7.0, line.center().y), Vec2::splat(14.0)),
                row.kind.icon(),
                chrome.text_faint,
            );

            // 动作按钮占右边，剩下的才是名字的宽度：名字长了在它前面截断。
            let actions_right = line.max.x - tokens.gap_l;
            let purge_rect = Rect::from_min_size(
                egui::pos2(
                    actions_right - ACTION,
                    line.center().y - tokens.control_height / 2.0,
                ),
                Vec2::new(ACTION, tokens.control_height),
            );
            let restore_rect = Rect::from_min_size(
                egui::pos2(purge_rect.min.x - tokens.gap_s - ACTION, purge_rect.min.y),
                Vec2::new(ACTION, tokens.control_height),
            );
            let meta_right = restore_rect.min.x - tokens.gap_l;
            let title_left = left + 30.0;
            let title_area = Rect::from_min_max(
                egui::pos2(title_left, line.min.y),
                egui::pos2(meta_right - tokens.gap_m, line.max.y),
            );
            painter.with_clip_rect(title_area).text(
                egui::pos2(title_left, line.center().y),
                Align2::LEFT_CENTER,
                if row.title.is_empty() {
                    strings::TRASH_UNTITLED
                } else {
                    row.title.as_str()
                },
                FontId::proportional(tokens.body),
                chrome.text,
            );
            painter.text(
                egui::pos2(meta_right, line.center().y),
                Align2::RIGHT_CENTER,
                strings::trash_meta(row.kind.label(), row.days_left, row.bytes),
                FontId::proportional(tokens.meta),
                chrome.text_faint,
            );

            if ui
                .put(
                    restore_rect,
                    Button::new(
                        RichText::new(strings::TRASH_RESTORE)
                            .size(tokens.meta)
                            .color(chrome.text_muted),
                    )
                    .frame(false),
                )
                .clicked()
            {
                restore = Some(row.folder.clone());
            }
            let armed = self.confirming.as_deref() == Some(row.folder.as_str());
            let (label, colour) = if armed {
                (strings::TRASH_PURGE_CONFIRM, chrome.danger)
            } else {
                (strings::TRASH_PURGE, chrome.text_muted)
            };
            if ui
                .put(
                    purge_rect,
                    Button::new(RichText::new(label).size(tokens.meta).color(colour)).frame(false),
                )
                .clicked()
            {
                if armed {
                    purge = Some(row.folder.clone());
                } else {
                    confirm = Some(row.folder.clone());
                }
            }
            y += ROW;
            if y > rect.max.y {
                break;
            }
        }

        if let Some(folder) = restore {
            self.confirming = None;
            action = Some(TrashAction::Restore(folder));
        }
        if let Some(folder) = purge {
            self.confirming = None;
            action = Some(TrashAction::Purge(folder));
        }
        if let Some(folder) = confirm {
            self.confirming = Some(folder);
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(folder: &str) -> TrashRow {
        TrashRow {
            folder: folder.to_owned(),
            kind: TrashKind::Board,
            title: folder.to_owned(),
            days_left: 7,
            bytes: 1024,
        }
    }

    #[test]
    fn a_vanished_row_drops_its_confirmation() {
        let mut view = TrashView::new();
        view.set_rows(vec![row("a"), row("b")]);
        view.confirming = Some("b".to_owned());
        view.set_rows(vec![row("a")]);
        assert!(view.confirming.is_none(), "确认的那条已经不在列表里了");
    }

    #[test]
    fn an_empty_list_drops_the_clear_confirmation() {
        let mut view = TrashView::new();
        view.set_rows(vec![row("a")]);
        view.confirm_empty = true;
        view.set_rows(Vec::new());
        assert!(!view.confirm_empty);
    }
}
