#![forbid(unsafe_code)]

//! 界面：把状态映射成画面。

pub mod board;
pub mod chrome;
pub mod strings;
pub mod theme;

pub use board::BoardApp;

use notecevia_core::version;

/// 窗口标题。
pub fn window_title() -> String {
    format!("Notecevia {}", version())
}
