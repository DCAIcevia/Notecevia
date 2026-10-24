#![forbid(unsafe_code)]

//! 领域模型与用例，不含 IO、时钟与界面类型。

pub mod board;

pub use board::{BlockKind, Board, Document, Entry, Geometry, TextBlock};

/// 当前 crate 版本。
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
