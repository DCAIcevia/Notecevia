#![forbid(unsafe_code)]

//! 文件读写、索引与格式迁移。

use notecevia_core::version;

/// 存储层是否可用。
pub fn available() -> bool {
    !version().is_empty()
}
