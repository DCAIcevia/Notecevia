//! 回收站命令：列出、还原、彻底删除、清空。
//!
//! 保留期与"还剩几天"都由 `store` 算，这里只把当前时刻传进去，前端拿到的就是能直接
//! 显示的数字。条目按 `.trash/` 下的目录名定位，与界面显示的名字无关。

use notecevia_store::Workspace;

use crate::ipc::now_seconds;

use super::dto::TrashItemDto;
use super::error::IpcError;
use super::state::AppState;

/// 回收站里能还原的东西，按删除时间从新到旧。
#[tauri::command]
pub fn list_trash(state: tauri::State<'_, AppState>) -> Result<Vec<TrashItemDto>, IpcError> {
    state.with(trash_items)
}

/// 把一条搬回原处；编号被占用时报错，不覆盖已有的东西。
#[tauri::command]
pub fn restore_trash(state: tauri::State<'_, AppState>, folder: String) -> Result<(), IpcError> {
    state.with(|workspace| Ok(workspace.restore_trash(&folder)?))
}

/// 立刻彻底删掉一条，不等保留期。
#[tauri::command]
pub fn purge_trash_item(state: tauri::State<'_, AppState>, folder: String) -> Result<(), IpcError> {
    state.with(|workspace| Ok(workspace.purge_trash_item(&folder)?))
}

/// 清空回收站，返回删掉的条数。
#[tauri::command]
pub fn empty_trash(state: tauri::State<'_, AppState>) -> Result<usize, IpcError> {
    state.with(|workspace| Ok(workspace.empty_trash()?))
}

/// 读出回收站，按现在的时刻算好每条还剩几天。
fn trash_items(workspace: &Workspace) -> Result<Vec<TrashItemDto>, IpcError> {
    let now = now_seconds();
    Ok(workspace
        .list_trash()?
        .iter()
        .map(|item| TrashItemDto::new(item, now))
        .collect())
}

#[cfg(test)]
mod tests {
    use notecevia_core::Document;

    use super::super::dto::TrashKindDto;
    use super::super::testing::TempWorkspace;
    use super::*;

    /// 存一张画板再删掉，返回它的编号。
    fn deleted_board(workspace: &Workspace, title: &str) -> u64 {
        let board_id = workspace.next_board().expect("下一个画板编号");
        workspace
            .save_at(board_id, &Document::new(title), 1_000)
            .expect("存画板");
        workspace.delete_board(board_id).expect("删画板");
        board_id
    }

    #[test]
    fn list_trash_shows_a_deleted_board_with_days_left() {
        let temp = TempWorkspace::new("trash-list");
        let board_id = deleted_board(&temp.workspace, "回收站的画板");

        let items = trash_items(&temp.workspace).expect("列回收站");

        assert_eq!(items.len(), 1, "刚删一张就应该只有一条");
        let item = &items[0];
        assert_eq!(item.kind, TrashKindDto::Board);
        assert_eq!(item.id, board_id);
        assert_eq!(item.title, "回收站的画板");
        assert!(
            (1..=30).contains(&item.days_left),
            "刚删的条目应该还剩 30 天以内：{}",
            item.days_left
        );
    }

    #[test]
    fn restore_puts_the_board_back_and_clears_the_list() {
        let temp = TempWorkspace::new("trash-restore");
        let board_id = deleted_board(&temp.workspace, "要还原的画板");
        let folder = trash_items(&temp.workspace).expect("列回收站")[0]
            .folder
            .clone();

        temp.workspace.restore_trash(&folder).expect("还原");

        assert!(
            trash_items(&temp.workspace).expect("列回收站").is_empty(),
            "还原后列表应该空"
        );
        let document = temp.workspace.load(board_id).expect("读还原后的画板");
        assert_eq!(document.board.title, "要还原的画板");
    }

    #[test]
    fn purge_removes_one_item_from_the_list() {
        let temp = TempWorkspace::new("trash-purge");
        deleted_board(&temp.workspace, "要彻底删的画板");
        let folder = trash_items(&temp.workspace).expect("列回收站")[0]
            .folder
            .clone();

        temp.workspace.purge_trash_item(&folder).expect("彻底删除");

        assert!(
            trash_items(&temp.workspace).expect("列回收站").is_empty(),
            "彻底删掉后列表应该空"
        );
    }

    #[test]
    fn empty_trash_reports_the_number_it_removed() {
        let temp = TempWorkspace::new("trash-empty");
        deleted_board(&temp.workspace, "第一张");
        deleted_board(&temp.workspace, "第二张");

        let removed = temp.workspace.empty_trash().expect("清空回收站");

        assert_eq!(removed, 2, "清掉的条数应该等于删掉的张数");
        assert!(trash_items(&temp.workspace).expect("列回收站").is_empty());
    }
}
