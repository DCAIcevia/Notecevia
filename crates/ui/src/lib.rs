#![forbid(unsafe_code)]
#![deny(rustdoc::broken_intra_doc_links)]

//! 界面：把状态映射成画面。

pub mod board;
pub mod chrome;
pub mod fonts;
pub mod home;
pub mod icons;
pub mod library;
pub mod media;
pub mod shell;
pub mod strings;
pub mod theme;

pub use board::BoardApp;
pub use home::{BoardItem, ExportKind, HomeAction, HomeApp, ThumbnailSource};
pub use library::{Library, LibraryAction, PageRow, VolumeRow};
pub use media::MediaSource;

use notecevia_core::version;

/// 窗口标题。
pub fn window_title() -> String {
    format!("Notecevia {}", version())
}
