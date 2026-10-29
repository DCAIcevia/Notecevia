//! 命令的错误形状：前端按 `kind` 分支，按 `message` 显示。

use notecevia_store::Error as StoreError;
use serde::Serialize;

/// 一条命令错误。`kind` 是稳定字符串，前端不要拿 `message` 做判断。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub kind: &'static str,
    pub message: String,
}

impl IpcError {
    pub fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// 这一层自己出的错（锁坏了、状态不对），不是内核报的。
    pub(crate) fn internal(message: impl Into<String>) -> Self {
        Self::new("internal", message)
    }

    /// 前端在 `open_workspace` 之前调了别的命令。
    pub(crate) fn workspace_not_open() -> Self {
        Self::new("workspaceNotOpen", "工作区还没打开，先调用 open_workspace")
    }
}

impl From<StoreError> for IpcError {
    fn from(error: StoreError) -> Self {
        let kind = match &error {
            StoreError::Io(_) => "io",
            StoreError::Cbor(_) | StoreError::MissingField(_) | StoreError::WrongType(_) => {
                "format"
            }
            StoreError::OutsideWorkspace(_) => "outsideWorkspace",
            StoreError::BadMediaName(_) => "badMediaName",
            StoreError::BadContainer(_) => "badContainer",
            StoreError::NewerFormat { .. } => "newerFormat",
            StoreError::CannotDeleteEmptyBook => "cannotDeleteEmptyBook",
            StoreError::BadTrashFolder(_) => "badTrash",
            StoreError::TrashTargetExists(_) => "trashTargetExists",
        };
        Self::new(kind, error.to_string())
    }
}
