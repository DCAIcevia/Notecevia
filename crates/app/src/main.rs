#![forbid(unsafe_code)]

//! 二进制入口：组装存储与界面。

fn main() {
    let _ = notecevia_store::available();
    let _ = notecevia_ui::window_title();
}
