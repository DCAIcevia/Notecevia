#![forbid(unsafe_code)]

//! 领域模型与用例，不含 IO、时钟与界面类型。

pub mod arrange;
pub mod board;
pub mod history;
pub mod volume;

pub use arrange::{Alignment, Axis};
pub use board::{
    BlockKind, Board, Body, Document, Entry, Geometry, HighlightBlock, ImageBlock, ImageFit,
    MediaBlock, ShapeBlock, ShapeKind, TableBlock, TextBlock,
};
pub use history::History;
pub use volume::{Shelf, Volume};

/// 当前 crate 版本。
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
