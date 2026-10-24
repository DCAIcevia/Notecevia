#![forbid(unsafe_code)]

//! 文件读写、索引与格式迁移。

pub mod cbor;
pub mod workspace;

pub use notecevia_core::Document;
pub use workspace::{Error, Workspace};

use std::path::PathBuf;

/// 工作区默认位置：用户目录下的 Notecevia 文件夹。
///
/// 不放 AppData：画板是文档，用户要能看见、能自己备份，卸载与清理也不该带走它。
/// 也不放文档目录：OneDrive 默认会重定向那里，等于把笔记悄悄同步上云。
/// 环境变量 NOTECEVIA_HOME 可以改位置，测试与排障都靠它。
pub fn workspace_root() -> PathBuf {
    if let Some(root) = std::env::var_os("NOTECEVIA_HOME") {
        return PathBuf::from(root);
    }
    for key in ["USERPROFILE", "HOME"] {
        if let Some(home) = std::env::var_os(key) {
            return PathBuf::from(home).join("Notecevia");
        }
    }
    PathBuf::from("Notecevia")
}
