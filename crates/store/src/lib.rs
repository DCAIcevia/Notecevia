#![forbid(unsafe_code)]

//! 文件读写、索引与格式迁移。

mod base64;
pub mod cbor;
mod container;
mod digest;
mod error;
mod format;
mod html;
mod index;
mod migrate;
mod paths;
mod transfer;
pub mod workspace;

pub use error::Error;
pub use index::{BoardInfo, SearchHit};
pub use notecevia_core::Document;
pub use workspace::Workspace;

use std::path::{Path, PathBuf};

/// 工作区放在程序所在目录下，随程序走。
///
/// 不放 AppData，也不放用户目录和文档目录：数据该跟着程序，整个文件夹拷走就换了机器。
/// 程序目录不可写时宁可在打开时报错让人看见，也不偷偷把数据写到别处——
/// 悄悄换了位置，用户以后找不到自己的笔记。
/// 环境变量 NOTECEVIA_HOME 可以覆盖，测试与排障都靠它。
pub fn workspace_root() -> PathBuf {
    if let Some(root) = std::env::var_os("NOTECEVIA_HOME") {
        return PathBuf::from(root);
    }
    match std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        Some(dir) => dir.join("NoteceviaData"),
        None => PathBuf::from("NoteceviaData"),
    }
}
