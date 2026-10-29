//! 册与书架的读写命令。
//!
//! 盘上的布局、页的归属规则都由 `notecevia_store` 负责；这一层只把参数换成内核类型、
//! 把结果换成 DTO。建册与建书架要写修改时间，时间在这里取一次传给 `store`。
//!
//! 命令本身只是 `state.with(...)` 的壳，逻辑放在下面的普通函数里，测试直接拿临时
//! 工作区调用它们。

use notecevia_core::{Document, Shelf, Volume};
use notecevia_store::Workspace;

use super::dto::{BoardSummary, PageCreatedDto, ShelfDto, ShelfSummary, VolumeDto, VolumeSummary};
use super::error::IpcError;
use super::now_seconds;
use super::state::AppState;

/// 册的列表。Empty book 固定排在最后。
#[tauri::command]
pub fn list_volumes(state: tauri::State<'_, AppState>) -> Result<Vec<VolumeSummary>, IpcError> {
    state.with(collect_volumes)
}

/// 打开一本册：标题与有序的页编号。
#[tauri::command]
pub fn load_volume(
    state: tauri::State<'_, AppState>,
    volume_id: u64,
) -> Result<VolumeDto, IpcError> {
    state.with(|workspace| open_volume(workspace, volume_id))
}

/// 整份存册。册里的页编号就是这些页的归属。
#[tauri::command]
pub fn save_volume(
    state: tauri::State<'_, AppState>,
    volume_id: u64,
    volume: VolumeDto,
) -> Result<(), IpcError> {
    state.with(|workspace| write_volume(workspace, volume_id, volume))
}

/// 新建一本空册，返回列表里要显示的那一行。
#[tauri::command]
pub fn create_volume(
    state: tauri::State<'_, AppState>,
    title: String,
) -> Result<VolumeSummary, IpcError> {
    state.with(|workspace| new_volume(workspace, title))
}

/// 删一本册：册里的页一起进回收站。Empty book 会返回 `cannotDeleteEmptyBook`。
#[tauri::command]
pub fn delete_volume(state: tauri::State<'_, AppState>, volume_id: u64) -> Result<(), IpcError> {
    state.with(|workspace| remove_volume(workspace, volume_id))
}

/// 把一页放进册的第 `at` 位，返回它原先所在的册。
#[tauri::command]
pub fn assign_page(
    state: tauri::State<'_, AppState>,
    volume_id: u64,
    page: u64,
    at: usize,
) -> Result<Option<u64>, IpcError> {
    state.with(|workspace| assign_volume_page(workspace, volume_id, page, at))
}

/// 把一页放回 Empty book，返回它原先所在的册。
#[tauri::command]
pub fn unassign_page(
    state: tauri::State<'_, AppState>,
    page: u64,
) -> Result<Option<u64>, IpcError> {
    state.with(|workspace| unassign_volume_page(workspace, page))
}

/// 把一页从一本册挪到另一本册。
#[tauri::command]
pub fn move_page(
    state: tauri::State<'_, AppState>,
    from_volume: u64,
    to_volume: u64,
    page: u64,
    at: usize,
) -> Result<(), IpcError> {
    state.with(|workspace| move_volume_page(workspace, from_volume, to_volume, page, at))
}

/// 册内重排：把第 `from` 页挪到第 `to` 位。
#[tauri::command]
pub fn move_page_in_volume(
    state: tauri::State<'_, AppState>,
    volume_id: u64,
    from: usize,
    to: usize,
) -> Result<(), IpcError> {
    state.with(|workspace| reorder_volume_pages(workspace, volume_id, from, to))
}

/// 一本册里的页，顺序取自册文件。
#[tauri::command]
pub fn pages_in_volume(
    state: tauri::State<'_, AppState>,
    volume_id: u64,
) -> Result<Vec<u64>, IpcError> {
    state.with(|workspace| volume_pages(workspace, volume_id))
}

/// 一页属于哪本册。页文件不在给 `null`，在盘上却没有归属算 Empty book。
#[tauri::command]
pub fn volume_of_page(
    state: tauri::State<'_, AppState>,
    page: u64,
) -> Result<Option<u64>, IpcError> {
    state.with(|workspace| page_volume(workspace, page))
}

/// 新建一页：给了册就排在末尾，没给就顺手建出一本同名册。
#[tauri::command]
pub fn create_page(
    state: tauri::State<'_, AppState>,
    title: String,
    volume_id: Option<u64>,
) -> Result<PageCreatedDto, IpcError> {
    state.with(|workspace| new_page(workspace, title, volume_id))
}

/// 书架的列表。
#[tauri::command]
pub fn list_shelves(state: tauri::State<'_, AppState>) -> Result<Vec<ShelfSummary>, IpcError> {
    state.with(collect_shelves)
}

/// 打开一层书架：标题与有序的册编号。
#[tauri::command]
pub fn load_shelf(state: tauri::State<'_, AppState>, shelf_id: u64) -> Result<ShelfDto, IpcError> {
    state.with(|workspace| open_shelf(workspace, shelf_id))
}

/// 整份存书架。书架只存册的引用，挪动不会碰册文件。
#[tauri::command]
pub fn save_shelf(
    state: tauri::State<'_, AppState>,
    shelf_id: u64,
    shelf: ShelfDto,
) -> Result<(), IpcError> {
    state.with(|workspace| write_shelf(workspace, shelf_id, shelf))
}

/// 新建一层空书架，返回列表里要显示的那一行。
#[tauri::command]
pub fn create_shelf(
    state: tauri::State<'_, AppState>,
    title: String,
) -> Result<ShelfSummary, IpcError> {
    state.with(|workspace| new_shelf(workspace, title))
}

/// 删一层书架：只移书架文件，册不动。
#[tauri::command]
pub fn delete_shelf(state: tauri::State<'_, AppState>, shelf_id: u64) -> Result<(), IpcError> {
    state.with(|workspace| remove_shelf(workspace, shelf_id))
}

/// 把一本册放到书架上：原来在别的架上会挪过来。
#[tauri::command]
pub fn shelve_volume(
    state: tauri::State<'_, AppState>,
    shelf_id: u64,
    volume_id: u64,
) -> Result<(), IpcError> {
    state.with(|workspace| place_on_shelf(workspace, shelf_id, volume_id))
}

/// 把一本册从书架上拿掉；拿空之后书架进回收站。
#[tauri::command]
pub fn unshelve_volume(
    state: tauri::State<'_, AppState>,
    shelf_id: u64,
    volume_id: u64,
) -> Result<(), IpcError> {
    state.with(|workspace| take_off_shelf(workspace, shelf_id, volume_id))
}

/// 用两本以上的册合成一层书架。
#[tauri::command]
pub fn create_shelf_with(
    state: tauri::State<'_, AppState>,
    title: String,
    volumes: Vec<u64>,
) -> Result<ShelfSummary, IpcError> {
    state.with(|workspace| combine_shelf(workspace, title, volumes))
}

/// 新建一页：指定册就排在末尾，没有就顺手建出一本同名册。
fn new_page(
    workspace: &Workspace,
    title: String,
    volume_id: Option<u64>,
) -> Result<PageCreatedDto, IpcError> {
    let board_id = workspace.next_board()?;
    let updated_at = now_seconds();
    let document = Document::new(title.clone());
    workspace.save_at(board_id, &document, updated_at)?;
    let volume = match volume_id {
        Some(id) => {
            workspace.assign_page(id, board_id, usize::MAX)?;
            let held = workspace.load_volume(id)?;
            VolumeSummary {
                id,
                title: held.title,
                page_count: held.pages.len(),
                updated_at,
            }
        }
        None => {
            let id = workspace.next_volume()?;
            let volume = Volume {
                title,
                pages: vec![board_id],
            };
            workspace.save_volume_at(id, &volume, updated_at)?;
            VolumeSummary {
                id,
                title: volume.title,
                page_count: volume.pages.len(),
                updated_at,
            }
        }
    };
    Ok(PageCreatedDto {
        board: BoardSummary {
            id: board_id,
            title: document.board.title,
            tags: document.board.tags,
            updated_at,
            entry_count: 0,
        },
        volume,
    })
}

/// 把册放到书架上：原来在别的架上会挪过来。
fn place_on_shelf(workspace: &Workspace, shelf_id: u64, volume_id: u64) -> Result<(), IpcError> {
    workspace.shelve_volume(shelf_id, volume_id)?;
    Ok(())
}

/// 把册从书架上拿掉；拿空之后书架进回收站。
fn take_off_shelf(workspace: &Workspace, shelf_id: u64, volume_id: u64) -> Result<(), IpcError> {
    workspace.unshelve_volume(shelf_id, volume_id)?;
    Ok(())
}

/// 用两本以上的册合成一层书架，返回列表里要显示的那一行。
fn combine_shelf(
    workspace: &Workspace,
    title: String,
    volumes: Vec<u64>,
) -> Result<ShelfSummary, IpcError> {
    Ok(ShelfSummary::from(
        &workspace.create_shelf_with(title, volumes)?,
    ))
}

/// 全部册，Empty book 固定排在最后。
fn collect_volumes(workspace: &Workspace) -> Result<Vec<VolumeSummary>, IpcError> {
    Ok(workspace
        .list_volumes()?
        .iter()
        .map(VolumeSummary::from)
        .collect())
}

/// 打开一本册。指向已删页的编号由 `store` 丢掉。
fn open_volume(workspace: &Workspace, volume_id: u64) -> Result<VolumeDto, IpcError> {
    Ok(VolumeDto::from(&workspace.load_volume(volume_id)?))
}

/// 整份存册。把一页写进这个册的编号表，就是把它从原来的册里摘掉。
fn write_volume(workspace: &Workspace, volume_id: u64, volume: VolumeDto) -> Result<(), IpcError> {
    workspace.save_volume(volume_id, &Volume::from(volume))?;
    Ok(())
}

/// 新建一本空册，返回 `{ id, title, page_count: 0, updated_at }`。
fn new_volume(workspace: &Workspace, title: String) -> Result<VolumeSummary, IpcError> {
    let volume_id = workspace.next_volume()?;
    let volume = Volume::new(title);
    let updated_at = now_seconds();
    workspace.save_volume_at(volume_id, &volume, updated_at)?;
    Ok(VolumeSummary {
        id: volume_id,
        title: volume.title,
        page_count: 0,
        updated_at,
    })
}

/// 删一本册：册里的页一起进回收站，Empty book 不能删。
fn remove_volume(workspace: &Workspace, volume_id: u64) -> Result<(), IpcError> {
    workspace.delete_volume(volume_id)?;
    Ok(())
}

/// 把一页放进册的第 `at` 位，返回它原先所在的册。
fn assign_volume_page(
    workspace: &Workspace,
    volume_id: u64,
    page: u64,
    at: usize,
) -> Result<Option<u64>, IpcError> {
    Ok(workspace.assign_page(volume_id, page, at)?)
}

/// 把一页放回 Empty book，返回它原先所在的册。
fn unassign_volume_page(workspace: &Workspace, page: u64) -> Result<Option<u64>, IpcError> {
    Ok(workspace.unassign_page(page)?)
}

/// 把一页从一本册挪到另一本册。归属以册文件为准，原来不在 `from_volume` 里也不报错。
fn move_volume_page(
    workspace: &Workspace,
    from_volume: u64,
    to_volume: u64,
    page: u64,
    at: usize,
) -> Result<(), IpcError> {
    workspace.move_page(from_volume, to_volume, page, at)?;
    Ok(())
}

/// 册内重排：把第 `from` 页挪到第 `to` 位，下标越界夹到边界。
fn reorder_volume_pages(
    workspace: &Workspace,
    volume_id: u64,
    from: usize,
    to: usize,
) -> Result<(), IpcError> {
    workspace.move_page_in_volume(volume_id, from, to)?;
    Ok(())
}

/// 一本册里的页，顺序取自册文件。
fn volume_pages(workspace: &Workspace, volume_id: u64) -> Result<Vec<u64>, IpcError> {
    Ok(workspace.pages_in_volume(volume_id)?)
}

/// 一页属于哪本册。页文件不在给 `None`，在盘上却没有归属算 Empty book。
fn page_volume(workspace: &Workspace, page: u64) -> Result<Option<u64>, IpcError> {
    Ok(workspace.volume_of_page(page)?)
}

/// 全部书架。
fn collect_shelves(workspace: &Workspace) -> Result<Vec<ShelfSummary>, IpcError> {
    Ok(workspace
        .list_shelves()?
        .iter()
        .map(ShelfSummary::from)
        .collect())
}

/// 打开一层书架。指向已删册的编号由 `store` 丢掉。
fn open_shelf(workspace: &Workspace, shelf_id: u64) -> Result<ShelfDto, IpcError> {
    Ok(ShelfDto::from(&workspace.load_shelf(shelf_id)?))
}

/// 整份存书架。书架只存册的引用，册文件不动。
fn write_shelf(workspace: &Workspace, shelf_id: u64, shelf: ShelfDto) -> Result<(), IpcError> {
    workspace.save_shelf(shelf_id, &Shelf::from(shelf))?;
    Ok(())
}

/// 新建一层空书架，返回 `{ id, title, volume_count: 0, updated_at }`。
fn new_shelf(workspace: &Workspace, title: String) -> Result<ShelfSummary, IpcError> {
    let shelf_id = workspace.next_shelf()?;
    let shelf = Shelf::new(title);
    let updated_at = now_seconds();
    workspace.save_shelf_at(shelf_id, &shelf, updated_at)?;
    Ok(ShelfSummary {
        id: shelf_id,
        title: shelf.title,
        volume_count: 0,
        updated_at,
    })
}

/// 删一层书架：只移书架文件，册不动。
fn remove_shelf(workspace: &Workspace, shelf_id: u64) -> Result<(), IpcError> {
    workspace.delete_shelf(shelf_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use notecevia_core::Document;
    use notecevia_core::volume::EMPTY_BOOK_ID;
    use notecevia_store::TrashKind;

    use super::*;
    use crate::ipc::testing::TempWorkspace;

    /// 固定时间的存盘时刻，测试不读系统时钟。
    const STAMP: i64 = 1_700_000_000;

    fn add_page(workspace: &Workspace, id: u64, title: &str) {
        workspace
            .save_at(id, &Document::new(title), STAMP)
            .expect("建页");
    }

    #[test]
    fn created_volume_lists_loads_and_saves() {
        let temp = TempWorkspace::new("volume-roundtrip");
        let created = new_volume(&temp.workspace, "第一册".to_owned()).expect("建册");
        assert_eq!(created.title, "第一册");
        assert_eq!(created.page_count, 0);

        let listed = collect_volumes(&temp.workspace).expect("列册");
        assert!(
            listed.iter().any(|row| row.id == created.id),
            "新建的册应该出现在列表里"
        );

        let mut loaded = open_volume(&temp.workspace, created.id).expect("读册");
        assert_eq!(loaded.title, "第一册");
        assert!(loaded.pages.is_empty());

        loaded.title = "改过的册".to_owned();
        write_volume(&temp.workspace, created.id, loaded).expect("存册");
        assert_eq!(
            open_volume(&temp.workspace, created.id)
                .expect("再读")
                .title,
            "改过的册"
        );
    }

    #[test]
    fn page_enters_volume_and_returns_to_empty_book() {
        let temp = TempWorkspace::new("volume-assign");
        let volume = new_volume(&temp.workspace, "册".to_owned()).expect("建册");
        add_page(&temp.workspace, 1, "页");

        // 页刚建出来还不属于任何册文件，按 Empty book 的页算。
        assert_eq!(
            assign_volume_page(&temp.workspace, volume.id, 1, 0).expect("入册"),
            Some(EMPTY_BOOK_ID)
        );
        assert_eq!(
            volume_pages(&temp.workspace, volume.id).expect("册内页"),
            vec![1]
        );
        assert_eq!(
            page_volume(&temp.workspace, 1).expect("页的册"),
            Some(volume.id)
        );

        assert_eq!(
            unassign_volume_page(&temp.workspace, 1).expect("摘页"),
            Some(volume.id)
        );
        assert!(
            volume_pages(&temp.workspace, volume.id)
                .expect("册内页")
                .is_empty()
        );
        assert_eq!(
            page_volume(&temp.workspace, 1).expect("页的册"),
            Some(EMPTY_BOOK_ID)
        );
    }

    #[test]
    fn deleting_volume_sends_pages_to_trash() {
        let temp = TempWorkspace::new("volume-delete");
        let volume = new_volume(&temp.workspace, "要删的册".to_owned()).expect("建册");
        add_page(&temp.workspace, 7, "页");
        assign_volume_page(&temp.workspace, volume.id, 7, 0).expect("入册");

        remove_volume(&temp.workspace, volume.id).expect("删册");

        assert!(
            !collect_volumes(&temp.workspace)
                .expect("列册")
                .iter()
                .any(|row| row.id == volume.id),
            "删掉的册不该再出现在列表里"
        );
        let trash = temp.workspace.list_trash().expect("列回收站");
        assert!(
            trash
                .iter()
                .any(|item| item.kind == TrashKind::Board && item.id == 7),
            "册里的页应该进回收站：{trash:?}"
        );
    }

    #[test]
    fn page_moves_between_volumes() {
        let temp = TempWorkspace::new("volume-move");
        let left = new_volume(&temp.workspace, "左".to_owned()).expect("建册");
        let right = new_volume(&temp.workspace, "右".to_owned()).expect("建册");
        add_page(&temp.workspace, 1, "页一");
        add_page(&temp.workspace, 2, "页二");
        assign_volume_page(&temp.workspace, left.id, 1, 0).expect("入左册");
        assign_volume_page(&temp.workspace, right.id, 2, 0).expect("入右册");

        move_volume_page(&temp.workspace, left.id, right.id, 1, 1).expect("挪页");

        assert!(
            volume_pages(&temp.workspace, left.id)
                .expect("左册")
                .is_empty()
        );
        assert_eq!(
            volume_pages(&temp.workspace, right.id).expect("右册"),
            vec![2, 1]
        );
        assert_eq!(
            page_volume(&temp.workspace, 1).expect("页的册"),
            Some(right.id)
        );
    }

    #[test]
    fn page_reorders_within_volume() {
        let temp = TempWorkspace::new("volume-reorder");
        let volume = new_volume(&temp.workspace, "册".to_owned()).expect("建册");
        for page in [11, 12, 13] {
            add_page(&temp.workspace, page, "页");
            assign_volume_page(&temp.workspace, volume.id, page, usize::MAX).expect("入册");
        }

        reorder_volume_pages(&temp.workspace, volume.id, 0, 2).expect("册内重排");

        assert_eq!(
            volume_pages(&temp.workspace, volume.id).expect("册内页"),
            vec![12, 13, 11]
        );
    }

    #[test]
    fn shelf_creates_lists_saves_and_deletes() {
        let temp = TempWorkspace::new("shelf-roundtrip");
        let volume = new_volume(&temp.workspace, "册".to_owned()).expect("建册");
        let created = new_shelf(&temp.workspace, "一层".to_owned()).expect("建书架");
        assert_eq!(created.title, "一层");
        assert_eq!(created.volume_count, 0);

        let listed = collect_shelves(&temp.workspace).expect("列书架");
        assert!(
            listed.iter().any(|row| row.id == created.id),
            "新建的书架应该出现在列表里"
        );

        let mut shelf = open_shelf(&temp.workspace, created.id).expect("读书架");
        assert!(shelf.volumes.is_empty());
        shelf.title = "改过的书架".to_owned();
        shelf.volumes.push(volume.id);
        write_shelf(&temp.workspace, created.id, shelf).expect("存书架");

        let loaded = open_shelf(&temp.workspace, created.id).expect("再读");
        assert_eq!(loaded.title, "改过的书架");
        assert_eq!(loaded.volumes, vec![volume.id]);

        remove_shelf(&temp.workspace, created.id).expect("删书架");
        assert!(
            collect_shelves(&temp.workspace).expect("再列").is_empty(),
            "删掉的书架不该再出现在列表里"
        );
        assert_eq!(
            open_shelf(&temp.workspace, created.id)
                .expect_err("读已删书架")
                .kind,
            "io"
        );
        assert!(
            collect_volumes(&temp.workspace)
                .expect("列册")
                .iter()
                .any(|row| row.id == volume.id),
            "删书架不该删掉册"
        );
    }

    #[test]
    fn empty_book_cannot_be_deleted() {
        let temp = TempWorkspace::new("empty-book");
        let error = remove_volume(&temp.workspace, EMPTY_BOOK_ID).expect_err("Empty book 不能删");
        assert_eq!(error.kind, "cannotDeleteEmptyBook");
    }

    #[test]
    fn page_created_without_context_brings_its_volume() {
        let temp = TempWorkspace::new("page-new-volume");
        let created = new_page(&temp.workspace, "新页".to_owned(), None).expect("建页");
        assert_eq!(created.board.title, "新页");
        assert_eq!(created.volume.title, "新页");
        assert_eq!(created.volume.page_count, 1);
        assert_eq!(
            volume_pages(&temp.workspace, created.volume.id).expect("册内页"),
            vec![created.board.id]
        );
    }

    #[test]
    fn page_created_inside_a_volume_lands_at_the_end() {
        let temp = TempWorkspace::new("page-in-volume");
        let volume = new_volume(&temp.workspace, "册".to_owned()).expect("建册");
        add_page(&temp.workspace, 1, "旧页");
        assign_volume_page(&temp.workspace, volume.id, 1, 0).expect("入册");

        let created = new_page(&temp.workspace, "新页".to_owned(), Some(volume.id)).expect("建页");
        assert_eq!(created.volume.id, volume.id);
        assert_eq!(
            volume_pages(&temp.workspace, volume.id).expect("册内页"),
            vec![1, created.board.id]
        );
    }
}
