#![forbid(unsafe_code)]

//! 回收站视图：删掉的东西摆一列，能还原，也能真删。
//!
//! 这里只管画和收动作：名字、剩余天数、大小都由 app 算好递进来，界面不碰文件，
//! 也不读时钟。彻底删除不可逆，所以点两下才算数。

use egui::{Align2, FontId, Rect, Ui, Vec2};

use crate::icons::Icon;
use crate::kit;
use crate::strings;
use crate::theme;

/// 头部高度：标题、规矩、清空按钮都在这条里。
const HEAD: f32 = 56.0;
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
        let mut action = None;

        // 头部：标题与保留期。说明写在这里，空态就不必再重复一遍。
        let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEAD));
        let left = rect.min.x + kit::INSET;
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

        // 右端是清空：不可逆，点两下才算数。
        if !self.rows.is_empty() {
            let (label, tone) = if self.confirm_empty {
                (strings::TRASH_EMPTY_CONFIRM, kit::Tone::Danger)
            } else {
                (strings::TRASH_EMPTY_ACTION, kit::Tone::Quiet)
            };
            let button = Rect::from_center_size(
                egui::pos2(
                    rect.max.x - kit::INSET - EMPTY_ACTION / 2.0,
                    head.center().y,
                ),
                Vec2::new(EMPTY_ACTION, kit::TOOL_ROW),
            );
            if kit::text_button(ui, button, label, tone).clicked() {
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

        // 一列：行的样子与状态由 kit 统一管，这里只说每行是什么。
        let shown: Vec<TrashRow> = self.rows.clone();
        let mut restore = None;
        let mut purge = None;
        let mut confirm = None;
        let mut rows = kit::Rows::new(
            Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + HEAD), rect.max),
            kit::ROW,
        );
        for row in &shown {
            let Some(area) = rows.take() else {
                break;
            };
            let armed = self.confirming.as_deref() == Some(row.folder.as_str());
            let meta = strings::trash_meta(row.kind.label(), row.days_left, row.bytes);
            let labels = [
                strings::TRASH_RESTORE,
                if armed {
                    strings::TRASH_PURGE_CONFIRM
                } else {
                    strings::TRASH_PURGE
                },
            ];
            let spec = kit::RowSpec {
                icon: Some(row.kind.icon()),
                label: if row.title.is_empty() {
                    strings::TRASH_UNTITLED
                } else {
                    row.title.as_str()
                },
                meta: &meta,
                actions: &labels,
                danger: Some(1),
                selected: false,
            };
            let hit = kit::row(ui, area, ("trash-row", row.folder.as_str()), &spec);
            match hit.action {
                Some(0) => restore = Some(row.folder.clone()),
                Some(_) if armed => purge = Some(row.folder.clone()),
                Some(_) => confirm = Some(row.folder.clone()),
                None => {}
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
