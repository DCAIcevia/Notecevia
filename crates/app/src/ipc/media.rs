//! 媒体与缩略图：二进制进出，不走 JSON。

use notecevia_store::Workspace;
use tauri::ipc::Response;

use super::error::IpcError;
use super::state::AppState;

pub(crate) fn read_bytes(workspace: &Workspace, hash: &str) -> Result<Vec<u8>, IpcError> {
    Ok(workspace.media_bytes(hash)?)
}

pub(crate) fn write_bytes(
    workspace: &Workspace,
    name: &str,
    data: &[u8],
) -> Result<String, IpcError> {
    Ok(workspace.put_media(name, data)?)
}

pub(crate) fn thumbnail(workspace: &Workspace, board_id: u64) -> Result<Vec<u8>, IpcError> {
    Ok(workspace.thumbnail_bytes(board_id)?.unwrap_or_default())
}

pub(crate) fn save_thumb(workspace: &Workspace, board_id: u64, png: &[u8]) -> Result<(), IpcError> {
    workspace.save_thumbnail(board_id, png)?;
    Ok(())
}

/// 读媒体原始字节。前端拿到 ArrayBuffer。
#[tauri::command]
pub fn read_media(state: tauri::State<'_, AppState>, hash: String) -> Result<Response, IpcError> {
    state.with(|workspace| Ok(Response::new(read_bytes(workspace, &hash)?)))
}

/// 写媒体：按内容哈希进媒体池，返回哈希。
///
/// 现在按 JSON 数字数组收字节，图片够用；视频这类大文件以后改成直接走原始请求体。
#[tauri::command]
pub fn write_media(
    state: tauri::State<'_, AppState>,
    name: String,
    data: Vec<u8>,
) -> Result<String, IpcError> {
    state.with(|workspace| write_bytes(workspace, &name, &data))
}

/// 读缩略图；没有缩略图时给空字节，前端按长度判断。
#[tauri::command]
pub fn read_thumbnail(
    state: tauri::State<'_, AppState>,
    board_id: u64,
) -> Result<Response, IpcError> {
    state.with(|workspace| Ok(Response::new(thumbnail(workspace, board_id)?)))
}

/// 写缩略图；生成 PNG 是界面层的事，这里只负责落盘。
#[tauri::command]
pub fn save_thumbnail(
    state: tauri::State<'_, AppState>,
    board_id: u64,
    png: Vec<u8>,
) -> Result<(), IpcError> {
    state.with(|workspace| save_thumb(workspace, board_id, &png))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::testing::TempWorkspace;

    #[test]
    fn media_roundtrips_by_content_hash() {
        let temp = TempWorkspace::new("media");
        let hash = write_bytes(&temp.workspace, "图.png", b"image-bytes").expect("写媒体");
        assert_eq!(
            read_bytes(&temp.workspace, &hash).expect("读媒体"),
            b"image-bytes"
        );

        // 同一份内容进池一次，哈希稳定。
        let again = write_bytes(&temp.workspace, "另一个名字.png", b"image-bytes").expect("写媒体");
        assert_eq!(again, hash);
    }

    #[test]
    fn missing_media_is_an_error_not_empty_bytes() {
        let temp = TempWorkspace::new("media-missing");
        let error = read_bytes(&temp.workspace, "not-a-hash").expect_err("不存在的媒体");
        assert_eq!(error.kind, "badMediaName");
    }

    #[test]
    fn thumbnail_can_be_written_read_and_missing_is_empty() {
        let temp = TempWorkspace::new("media-thumb");
        assert!(
            thumbnail(&temp.workspace, 1)
                .expect("没有缩略图")
                .is_empty()
        );
        save_thumb(&temp.workspace, 1, b"png-bytes").expect("写缩略图");
        assert_eq!(
            thumbnail(&temp.workspace, 1).expect("读缩略图"),
            b"png-bytes"
        );
    }
}
