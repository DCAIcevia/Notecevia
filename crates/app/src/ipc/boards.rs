//! 画板、列表、搜索与媒体索引的命令。

use notecevia_core::Document;
use notecevia_store::Workspace;

use super::dto::{BoardSummary, DocumentDto, MediaSummary, SaveResult, SearchHitDto};
use super::error::IpcError;
use super::now_seconds;
use super::state::AppState;

pub(crate) fn list(workspace: &Workspace) -> Result<Vec<BoardSummary>, IpcError> {
    Ok(workspace
        .list_boards()?
        .iter()
        .map(BoardSummary::from)
        .collect())
}

pub(crate) fn search(workspace: &Workspace, query: &str) -> Result<Vec<SearchHitDto>, IpcError> {
    Ok(workspace
        .search(query)?
        .iter()
        .map(SearchHitDto::from)
        .collect())
}

pub(crate) fn media(workspace: &Workspace) -> Result<Vec<MediaSummary>, IpcError> {
    Ok(workspace
        .list_media()?
        .iter()
        .map(MediaSummary::from)
        .collect())
}

pub(crate) fn open(workspace: &Workspace, board_id: u64) -> Result<DocumentDto, IpcError> {
    let document = workspace.load(board_id)?;
    Ok(DocumentDto::from(&document))
}

pub(crate) fn save(
    workspace: &Workspace,
    board_id: u64,
    document: &DocumentDto,
) -> Result<SaveResult, IpcError> {
    let document = Document::from(document.clone());
    let updated_at = now_seconds();
    workspace.save_at(board_id, &document, updated_at)?;
    Ok(SaveResult { updated_at })
}

pub(crate) fn create(workspace: &Workspace, title: String) -> Result<BoardSummary, IpcError> {
    let board_id = workspace.next_board()?;
    let document = Document::new(title);
    let updated_at = now_seconds();
    workspace.save_at(board_id, &document, updated_at)?;
    Ok(BoardSummary {
        id: board_id,
        title: document.board.title,
        tags: document.board.tags,
        updated_at,
        entry_count: 0,
    })
}

pub(crate) fn rename(workspace: &Workspace, board_id: u64, title: String) -> Result<(), IpcError> {
    let mut document = workspace.load(board_id)?;
    document.board.title = title;
    workspace.save_at(board_id, &document, now_seconds())?;
    Ok(())
}

pub(crate) fn delete(workspace: &Workspace, board_id: u64) -> Result<(), IpcError> {
    workspace.delete_board(board_id)?;
    Ok(())
}

/// 列表页：按修改时间从新到旧。
#[tauri::command]
pub fn list_boards(state: tauri::State<'_, AppState>) -> Result<Vec<BoardSummary>, IpcError> {
    state.with(list)
}

/// 搜索标题、标签、正文、表格单元格与媒体文件名。
#[tauri::command]
pub fn search_boards(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<SearchHitDto>, IpcError> {
    state.with(|workspace| search(workspace, &query))
}

/// 强制重建索引；外部程序改过工作区后用。
#[tauri::command]
pub fn rebuild_index(state: tauri::State<'_, AppState>) -> Result<usize, IpcError> {
    state.with(|workspace| Ok(workspace.rebuild_index()?))
}

/// 资源库：媒体池里的每一份内容。
#[tauri::command]
pub fn list_media(state: tauri::State<'_, AppState>) -> Result<Vec<MediaSummary>, IpcError> {
    state.with(media)
}

/// 下一个可用的画板编号。
#[tauri::command]
pub fn next_board_id(state: tauri::State<'_, AppState>) -> Result<u64, IpcError> {
    state.with(|workspace| Ok(workspace.next_board()?))
}

/// 打开一张画板：目录项与正文一起给。
#[tauri::command]
pub fn open_board(
    state: tauri::State<'_, AppState>,
    board_id: u64,
) -> Result<DocumentDto, IpcError> {
    state.with(|workspace| open(workspace, board_id))
}

/// 整份存盘，返回这次写盘用的修改时间。
#[tauri::command]
pub fn save_board(
    state: tauri::State<'_, AppState>,
    board_id: u64,
    document: DocumentDto,
) -> Result<SaveResult, IpcError> {
    state.with(|workspace| save(workspace, board_id, &document))
}

/// 新建一张空画板，返回列表行。
#[tauri::command]
pub fn create_board(
    state: tauri::State<'_, AppState>,
    title: String,
) -> Result<BoardSummary, IpcError> {
    state.with(|workspace| create(workspace, title))
}

/// 只改标题。
#[tauri::command]
pub fn rename_board(
    state: tauri::State<'_, AppState>,
    board_id: u64,
    title: String,
) -> Result<(), IpcError> {
    state.with(|workspace| rename(workspace, board_id, title))
}

/// 删一张画板：文件与缩略图一起进回收站，媒体留着。
#[tauri::command]
pub fn delete_board(state: tauri::State<'_, AppState>, board_id: u64) -> Result<(), IpcError> {
    state.with(|workspace| delete(workspace, board_id))
}

#[cfg(test)]
mod tests {
    use notecevia_core::{BlockKind, Geometry};

    use super::*;
    use crate::ipc::testing::TempWorkspace;

    #[test]
    fn created_board_shows_up_in_the_list_and_opens_empty() {
        let temp = TempWorkspace::new("boards-create");
        let created = create(&temp.workspace, "第一篇".to_owned()).expect("建画板");

        let listed = list(&temp.workspace).expect("列表");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
        assert_eq!(listed[0].title, "第一篇");
        assert_eq!(listed[0].entry_count, 0);

        let opened = open(&temp.workspace, created.id).expect("打开");
        assert_eq!(opened.board.title, "第一篇");
        assert!(opened.entries.is_empty());
        assert!(opened.bodies.is_empty());
    }

    #[test]
    fn saving_a_board_roundtrips_entries_and_bodies() {
        let temp = TempWorkspace::new("boards-save");
        let created = create(&temp.workspace, "存盘".to_owned()).expect("建画板");

        let mut document = Document::new("存盘");
        let text = document.add_text(Geometry::new(10.0, 20.0, 100.0, 40.0));
        document.text_mut(text).text = "正文".to_owned();
        document.add_block(BlockKind::Shape, Geometry::new(0.0, 0.0, 10.0, 10.0));
        let dto = DocumentDto::from(&document);

        let saved = save(&temp.workspace, created.id, &dto).expect("存盘");
        assert!(saved.updated_at > 0);

        let opened = open(&temp.workspace, created.id).expect("重新打开");
        assert_eq!(opened.entries.len(), 2);
        assert_eq!(opened.bodies.len(), 2);
        let restored = Document::from(opened);
        assert_eq!(restored.text_of(text), "正文");
    }

    #[test]
    fn rename_changes_the_title_and_keeps_content() {
        let temp = TempWorkspace::new("boards-rename");
        let created = create(&temp.workspace, "旧标题".to_owned()).expect("建画板");
        rename(&temp.workspace, created.id, "新标题".to_owned()).expect("改名");

        let opened = open(&temp.workspace, created.id).expect("打开");
        assert_eq!(opened.board.title, "新标题");
    }

    #[test]
    fn search_finds_text_written_into_a_board() {
        let temp = TempWorkspace::new("boards-search");
        let created = create(&temp.workspace, "可搜索".to_owned()).expect("建画板");
        let mut document = Document::new("可搜索");
        let text = document.add_text(Geometry::new(0.0, 0.0, 100.0, 40.0));
        document.text_mut(text).text = "这里有一个关键词".to_owned();
        save(&temp.workspace, created.id, &DocumentDto::from(&document)).expect("存盘");

        let hits = search(&temp.workspace, "关键词").expect("搜索");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].board_id, created.id);
    }

    #[test]
    fn delete_removes_the_board_from_the_list() {
        let temp = TempWorkspace::new("boards-delete");
        let created = create(&temp.workspace, "要删的".to_owned()).expect("建画板");
        delete(&temp.workspace, created.id).expect("删画板");
        assert!(list(&temp.workspace).expect("列表").is_empty());
    }
}
