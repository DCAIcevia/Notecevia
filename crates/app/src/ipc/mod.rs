//! Web 前端与 Rust 内核之间的命令层。
//!
//! 命令本身不装业务：把参数换成内核类型、调用 `core`／`store`、把错误转成可序列化的
//! [IpcError]。可测的逻辑放在各模块的普通函数里，命令只是 `state.with(...)` 的壳。
//!
//! 时间属于应用边界：存盘、回收站倒计时都在这里取 Unix 秒，内核与 `store` 各自管自己的时钟。

pub mod boards;
pub mod config;
pub mod decoration;
pub mod dto;
pub mod error;
pub mod health;
pub mod library;
pub mod media;
pub mod state;
pub mod transfer;
pub mod trash;

#[cfg(test)]
pub(crate) mod testing;

pub use error::IpcError;
pub use state::{AppInfo, AppState, WorkspaceStatus};

use std::time::{SystemTime, UNIX_EPOCH};

/// 当前 Unix 秒。取不到系统时间时给 0：宁可时间戳退化成 1970，也不要 panic。
pub(crate) fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or_default()
}
