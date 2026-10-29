//! 读写工作区会失败的地方，都收在这里，调用方按变体判断是哪一类。

use thiserror::Error;

use crate::cbor::CborError;

#[derive(Debug, Error)]
pub enum Error {
    #[error("读写工作区失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("文件内容读不懂：{0}")]
    Cbor(#[from] CborError),
    #[error("画板文件缺少字段：{0}")]
    MissingField(&'static str),
    #[error("画板文件里 {0} 的值不对")]
    WrongType(&'static str),
    #[error("地址不在工作区内，已拒绝：{0}")]
    OutsideWorkspace(String),
    #[error("媒体文件名不是内容哈希：{0}")]
    BadMediaName(String),
    #[error("容器文件读不懂：{0}")]
    BadContainer(&'static str),
    #[error("这个文件由更高版本的程序写入（格式 {found}，本程序支持 {supported}），请升级程序")]
    NewerFormat { found: i64, supported: i64 },
    #[error("Empty book 是默认册，不能删除")]
    CannotDeleteEmptyBook,
    #[error("一层书架至少要有两本册才能合成")]
    ShelfTooSmall,
    #[error("回收站里的这个名字不认：{0}")]
    BadTrashFolder(String),
    #[error("编号 {0} 已经有别的东西在用了，这条还原不了：先处理占用它的那条")]
    TrashTargetExists(u64),
}
