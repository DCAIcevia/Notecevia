//! 撤销与重做：以整份文档快照为单位。
//!
//! 快照比命令式简单：任何改动都不用先写成一条命令，界面在改之前调一次
//! `History::apply` 就行。代价是每次改动复制一份文档；正文只有文字和哈希，
//! 在 Alpha 的画板规模下可以接受。将来要省内存，再换成按块记差异。

use crate::board::Document;

/// 默认保留多少步。
const DEFAULT_LIMIT: usize = 100;

/// 撤销栈。
#[derive(Debug)]
pub struct History {
    past: Vec<Document>,
    future: Vec<Document>,
    limit: usize,
}

impl History {
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_LIMIT)
    }

    pub fn with_limit(limit: usize) -> Self {
        Self {
            past: Vec::new(),
            future: Vec::new(),
            limit: limit.max(1),
        }
    }

    /// 改一次文档。改完和改前一样就不记，避免空步骤占满栈。
    pub fn apply(&mut self, document: &mut Document, change: impl FnOnce(&mut Document)) {
        let before = document.clone();
        change(document);
        if *document == before {
            return;
        }
        self.past.push(before);
        if self.past.len() > self.limit {
            self.past.remove(0);
        }
        self.future.clear();
    }

    /// 退一步。没有可退的返回 false。
    pub fn undo(&mut self, document: &mut Document) -> bool {
        let Some(previous) = self.past.pop() else {
            return false;
        };
        let current = std::mem::replace(document, previous);
        self.future.push(current);
        true
    }

    /// 前进一步。没有可前进的返回 false。
    pub fn redo(&mut self, document: &mut Document) -> bool {
        let Some(next) = self.future.pop() else {
            return false;
        };
        let current = std::mem::replace(document, next);
        self.past.push(current);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Geometry;

    fn titled(title: &str) -> Document {
        Document::new(title)
    }

    #[test]
    fn undo_restores_the_previous_state() {
        let mut document = titled("画板");
        let mut history = History::new();
        history.apply(&mut document, |document| {
            document.add_text(Geometry::new(0.0, 0.0, 10.0, 10.0));
        });
        assert_eq!(document.board.entries.len(), 1);
        assert!(history.undo(&mut document));
        assert_eq!(document.board.entries.len(), 0);
        assert!(history.can_redo());
    }

    #[test]
    fn redo_returns_to_the_latest_state() {
        let mut document = titled("画板");
        let mut history = History::new();
        history.apply(&mut document, |document| {
            document.add_text(Geometry::new(0.0, 0.0, 10.0, 10.0));
        });
        history.undo(&mut document);
        assert!(history.redo(&mut document));
        assert_eq!(document.board.entries.len(), 1);
    }

    #[test]
    fn identical_change_is_not_recorded() {
        let mut document = titled("画板");
        let mut history = History::new();
        history.apply(&mut document, |_| {});
        assert!(!history.can_undo());
    }

    #[test]
    fn new_change_drops_the_redo_branch() {
        let mut document = titled("画板");
        let mut history = History::new();
        history.apply(&mut document, |document| {
            document.add_text(Geometry::new(0.0, 0.0, 10.0, 10.0));
        });
        history.undo(&mut document);
        history.apply(&mut document, |document| {
            document.add_text(Geometry::new(5.0, 5.0, 10.0, 10.0));
        });
        assert!(!history.can_redo());
    }

    #[test]
    fn limit_drops_the_oldest_step() {
        let mut document = titled("画板");
        let mut history = History::with_limit(2);
        for step in 0..4 {
            let x = step as f32 * 10.0;
            history.apply(&mut document, |document| {
                document.add_text(Geometry::new(x, 0.0, 10.0, 10.0));
            });
        }
        assert!(history.undo(&mut document));
        assert!(history.undo(&mut document));
        assert!(!history.undo(&mut document));
        assert_eq!(document.board.entries.len(), 2);
    }

    #[test]
    fn undo_without_history_is_a_no_op() {
        let mut document = titled("画板");
        let mut history = History::new();
        assert!(!history.undo(&mut document));
        assert!(!history.redo(&mut document));
    }
}
