//! 回收站：保留 30 天、列出来、还原、彻底删。坏条目只跳过它自己，下次再试。

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notecevia_core::board::{Body, Entry, Geometry, TextBlock};
use notecevia_core::{BlockKind, Document};
use notecevia_store::{Error, TrashKind, Workspace, cbor};

use support::TempWorkspace;

const DAY: i64 = 24 * 60 * 60;

/// 存一张带一块正文的笔记：还原之后正文也要跟着回来，所以正文必写。
fn save_note(temp: &TempWorkspace, id: u64, title: &str, text: &str) {
    let mut document = Document::new(title);
    document.board.entries.push(Entry::new(
        1,
        BlockKind::Text,
        Geometry::new(40.0, 40.0, 240.0, 120.0),
    ));
    document.bodies.insert(
        1,
        Body::Text(TextBlock {
            text: text.to_owned(),
            ..Default::default()
        }),
    );
    temp.workspace.save(id, &document).expect("存盘");
}

/// 当前秒级时间戳。测"打开工作区会不会清"时用，和 store 内部取的是同一个钟。
fn now_seconds() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("系统时间在 1970 之后");
    i64::try_from(elapsed.as_secs()).expect("时间戳放得进 i64")
}

/// 在回收站里放一个目录条目，名字由调用方定。返回条目路径。
fn trash_dir(temp: &TempWorkspace, name: &str) -> PathBuf {
    let dir = temp.path(".trash").join(name);
    fs::create_dir_all(&dir).expect("建回收站条目");
    fs::write(dir.join("1.board"), b"board").expect("放一个文件进去");
    dir
}

#[test]
fn entries_older_than_thirty_days_are_purged_and_younger_ones_kept() {
    let temp = TempWorkspace::new("trash-expiry");
    let now = 2_000_000_000;
    let expired = trash_dir(&temp, &format!("{}-board-1", now - 31 * DAY));
    let fresh = trash_dir(&temp, &format!("{}-board-2", now - 29 * DAY));

    let purged = temp.workspace.purge_trash(now).expect("清理回收站");

    assert_eq!(purged, 1, "只清过期的那一个");
    assert!(!expired.exists(), "31 天前删的该删掉");
    assert!(fresh.exists(), "29 天前删的要留着");
}

#[test]
fn exactly_thirty_days_old_is_not_purged_yet() {
    let temp = TempWorkspace::new("trash-boundary");
    let now = 2_000_000_000;
    let boundary = trash_dir(&temp, &format!("{}-volume-7", now - 30 * DAY));

    assert_eq!(temp.workspace.purge_trash(now).expect("清理回收站"), 0);
    assert!(boundary.exists(), "正好 30 天算还没到期");
}

#[test]
fn a_name_without_a_timestamp_falls_back_to_the_modified_time() {
    let temp = TempWorkspace::new("trash-mtime");
    let now = now_seconds();
    let trash = temp.path(".trash");
    fs::create_dir_all(&trash).expect("建回收站目录");
    // old 名字里没有时间戳；young 名字里带数字，用来证明没有去猜数字。
    let old = trash.join("handmade-old");
    let young = trash.join("handmade-12345");
    fs::write(&old, b"old").expect("写旧条目");
    fs::write(&young, b"young").expect("写新条目");
    let old_time = UNIX_EPOCH + Duration::from_secs(u64::try_from(now - 40 * DAY).expect("正数"));
    let handle = fs::File::options()
        .write(true)
        .open(&old)
        .expect("打开旧条目");
    handle
        .set_times(fs::FileTimes::new().set_modified(old_time))
        .expect("把修改时间改到 40 天前");

    let purged = temp.workspace.purge_trash(now).expect("清理回收站");

    assert_eq!(purged, 1, "只清修改时间过期的那个");
    assert!(!old.exists(), "名字读不出时间戳时按修改时间判断");
    assert!(young.exists(), "名字里的数字不该被当成时间戳");
}

#[test]
fn an_empty_trash_is_not_an_error() {
    let temp = TempWorkspace::new("trash-empty");
    // 新工作区没删过东西，没有 .trash 目录。
    assert!(!temp.path(".trash").exists());
    assert_eq!(
        temp.workspace
            .purge_trash(now_seconds())
            .expect("没有回收站目录"),
        0
    );
    fs::create_dir_all(temp.path(".trash")).expect("建空回收站");
    assert_eq!(
        temp.workspace.purge_trash(now_seconds()).expect("空回收站"),
        0
    );
}

#[test]
fn opening_a_workspace_purges_expired_entries_and_reports_the_count() {
    let temp = TempWorkspace::new("trash-on-open");
    let now = now_seconds();
    // 1970 年删的：哪一次打开都算过期。
    let expired = trash_dir(&temp, "1-board-9");
    let fresh = trash_dir(&temp, &format!("{}-board-8", now - DAY));

    let reopened = Workspace::open(&temp.root).expect("重开工作区");

    assert!(!expired.exists());
    assert!(fresh.exists());
    assert_eq!(reopened.purged_on_open(), 1);
}

/// 让一个回收站条目在这次清理里删不掉。两边的办法不同，测的是同一件事：
/// 删不掉的条目要跳过，不能中断清理，也不能让打开工作区失败。
#[cfg(windows)]
fn with_undeletable_entry<T>(dir: &Path, body: impl FnOnce() -> T) -> T {
    use std::os::windows::fs::OpenOptionsExt;

    /// FILE_SHARE_READ：不共享删除权限，占着它的文件就删不掉。
    const FILE_SHARE_READ: u32 = 0x001;

    let file = dir.join("locked.block");
    fs::write(&file, b"locked").expect("写被占住的文件");
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&file)
        .expect("占住文件句柄");
    let outcome = body();
    drop(held);
    outcome
}

#[cfg(unix)]
fn with_undeletable_entry<T>(dir: &Path, body: impl FnOnce() -> T) -> T {
    use std::os::unix::fs::PermissionsExt;

    fs::write(dir.join("locked.block"), b"locked").expect("写被占住的文件");
    // 删子项要目录的写权限，去掉写位就删不干净。
    fs::set_permissions(dir, fs::Permissions::from_mode(0o555)).expect("去掉目录写位");
    let outcome = body();
    let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o755));
    outcome
}

#[test]
fn an_undeletable_entry_is_skipped_and_does_not_fail_open() {
    let temp = TempWorkspace::new("trash-stuck");
    let now = now_seconds();
    let expired = trash_dir(&temp, &format!("{}-board-1", now - 40 * DAY));
    let stuck = trash_dir(&temp, &format!("{}-board-2", now - 40 * DAY));
    let fresh = trash_dir(&temp, &format!("{}-board-3", now - DAY));

    let purged = with_undeletable_entry(&stuck, || temp.workspace.purge_trash(now))
        .expect("一个坏条目不该让清理整段失败");

    assert_eq!(purged, 1, "清得掉的只有没被占住的那个");
    assert!(!expired.exists());
    assert!(stuck.exists(), "删不掉的条目要留下，下次再试");
    assert!(fresh.exists());

    let reopened = with_undeletable_entry(&stuck, || Workspace::open(&temp.root))
        .expect("删不掉的条目不该让打开工作区失败");
    assert_eq!(reopened.purged_on_open(), 0, "这一轮什么都没清掉");
    assert!(stuck.exists(), "下次再试：坏条目还在");
}

#[test]
fn a_deleted_note_comes_back_with_its_block() {
    let temp = TempWorkspace::new("trash-restore-board");
    let id = temp.workspace.next_board().expect("下一个编号");
    save_note(&temp, id, "可还原的笔记", "正文还在");
    temp.workspace.delete_board(id).expect("删掉");

    let items = temp.workspace.list_trash().expect("列出回收站");
    assert_eq!(items.len(), 1, "删一张笔记只该出一条");
    assert_eq!(items[0].kind, TrashKind::Board);
    assert_eq!(items[0].id, id);
    assert_eq!(
        items[0].title, "可还原的笔记",
        "名字从回收站里的画板文件读出来"
    );
    assert!(items[0].bytes > 0, "大小要能量出来");

    temp.workspace
        .restore_trash(&items[0].folder)
        .expect("还原");

    let document = temp.workspace.load(id).expect("读回来");
    assert_eq!(document.board.title, "可还原的笔记");
    assert!(
        matches!(document.bodies.get(&1), Some(Body::Text(text)) if text.text == "正文还在"),
        "块文件要跟着回来，光有画板文件不算还原"
    );
    assert!(
        temp.workspace
            .list_boards()
            .expect("列出来")
            .iter()
            .any(|board| board.id == id),
        "还原之后首页立刻能看到它"
    );
    assert!(
        temp.workspace.list_trash().expect("再列一次").is_empty(),
        "还原走的条目不该留在回收站里"
    );
}

#[test]
fn restore_refuses_when_the_number_is_taken_again() {
    let temp = TempWorkspace::new("trash-number-taken");
    let id = temp.workspace.next_board().expect("下一个编号");
    save_note(&temp, id, "旧的", "旧正文");
    temp.workspace.delete_board(id).expect("删掉");
    let folder = temp
        .workspace
        .list_trash()
        .expect("列出回收站")
        .remove(0)
        .folder;
    // 删掉最大的编号之后，下一个新笔记会拿到同一个号。
    save_note(&temp, id, "新的", "新正文");

    let outcome = temp.workspace.restore_trash(&folder);

    assert!(
        matches!(outcome, Err(Error::TrashTargetExists(taken)) if taken == id),
        "编号被占用时要明确拒绝，不能悄悄覆盖"
    );
    assert_eq!(
        temp.workspace.load(id).expect("读新的").board.title,
        "新的",
        "拒绝还原时不能动现在这条"
    );
    assert_eq!(
        temp.workspace.list_trash().expect("列出回收站").len(),
        1,
        "拒绝之后条目还在，用户还能再试"
    );
}

#[test]
fn purge_removes_one_entry_and_leaves_the_others() {
    let temp = TempWorkspace::new("trash-purge-one");
    for title in ["第一条", "第二条"] {
        let id = temp.workspace.next_board().expect("下一个编号");
        save_note(&temp, id, title, "正文");
        temp.workspace.delete_board(id).expect("删掉");
    }
    let items = temp.workspace.list_trash().expect("列出回收站");
    assert_eq!(items.len(), 2);

    temp.workspace
        .purge_trash_item(&items[0].folder)
        .expect("彻底删掉一条");

    let left = temp.workspace.list_trash().expect("再列一次");
    assert_eq!(left.len(), 1, "只该少一条");
    assert_eq!(left[0].folder, items[1].folder);
    // 剩下的那条还能正常还原，说明彻底删没伤到别人。
    temp.workspace.restore_trash(&left[0].folder).expect("还原");
    assert!(temp.workspace.list_trash().expect("列出回收站").is_empty());
}

#[test]
fn empty_trash_removes_everything_including_orphan_blocks() {
    let temp = TempWorkspace::new("trash-empty-all");
    let id = temp.workspace.next_board().expect("下一个编号");
    save_note(&temp, id, "要被清空的笔记", "正文");
    temp.workspace.delete_board(id).expect("删掉");
    let orphan = temp.path(".trash/1700000000-blocks");
    fs::create_dir_all(&orphan).expect("建孤儿块条目");
    fs::write(orphan.join(format!("{id}-9.block")), b"orphan").expect("写孤儿块");

    let removed = temp.workspace.empty_trash().expect("清空");

    assert_eq!(removed, 2, "笔记一条、孤儿块一条");
    assert!(temp.workspace.list_trash().expect("列出回收站").is_empty());
    assert!(!orphan.exists());
    assert_eq!(temp.workspace.empty_trash().expect("空回收站"), 0);
}

#[test]
fn orphan_block_entries_are_not_offered_for_restore() {
    let temp = TempWorkspace::new("trash-orphan-blocks");
    let orphan = temp.path(".trash/1700000000-blocks");
    fs::create_dir_all(&orphan).expect("建孤儿块条目");
    fs::write(orphan.join("1-1.block"), b"orphan").expect("写孤儿块");

    assert!(
        temp.workspace.list_trash().expect("列出回收站").is_empty(),
        "孤儿块是保存时的中间产物，不往用户面前摆"
    );
}

#[test]
fn a_trashed_volume_comes_back_to_the_volumes_folder() {
    let temp = TempWorkspace::new("trash-restore-volume");
    let folder = temp.path(".trash/1700000000-volume-3");
    fs::create_dir_all(&folder).expect("建回收站条目");
    // 还原只搬文件、不解析内容；列出来时要读名字，所以标题得写进去。
    let value = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(1)),
        ("kind", cbor::Value::text("volume")),
        ("title", cbor::Value::text("一本册")),
        ("pages", cbor::Value::Array(Vec::new())),
        ("updated_at", cbor::Value::Integer(0)),
    ]);
    fs::write(folder.join("3.volume"), cbor::encode(&value)).expect("写册文件");

    let items = temp.workspace.list_trash().expect("列出回收站");
    assert_eq!(items[0].kind, TrashKind::Volume);
    assert_eq!(items[0].title, "一本册");

    temp.workspace
        .restore_trash(&items[0].folder)
        .expect("还原");

    assert!(
        temp.path("volumes/3.volume").exists(),
        "册要回到 volumes/ 里"
    );
}

#[test]
fn a_folder_name_with_a_separator_is_refused() {
    let temp = TempWorkspace::new("trash-bad-name");
    assert!(temp.workspace.restore_trash("../boards").is_err());
    assert!(temp.workspace.purge_trash_item("../boards").is_err());
    assert!(temp.workspace.restore_trash("..").is_err());
}
