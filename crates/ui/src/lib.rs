#![forbid(unsafe_code)]

//! 界面、主题与交互。

use notecevia_core::version;

/// 窗口标题。
pub fn window_title() -> String {
    format!("Notecevia {}", version())
}
