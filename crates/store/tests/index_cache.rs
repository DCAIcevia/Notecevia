//! 首页索引：重建、增量更新、删除、外部改动与损坏恢复。
//!
//! 索引只是缓存，画板文件才是事实：这些用例都同时检查内容没被索引影响。

mod support;

use std::fs;

use notecevia_core::board::{BlockKind, Body, Document, Geometry};
use notecevia_store::{Workspace, cbor};

use support::TempWorkspace;

fn text_document(title: &str, body: &str) -> Document {
    let mut document = Document::new(title);
    let id = document.add_block(BlockKind::Text, Geometry::new(0.0, 0.0, 100.0, 40.0));
    if let Some(Body::Text(block)) = document.body_mut(id) {
        block.text = body.to_owned();
    }
    document
}

/// 手写一张最小格式 2 画板，模拟外部程序直接改文件。
fn external_board(title: &str, updated_at: i64) -> cbor::Value {
    cbor::Value::map(vec![
        ("format", cbor::Value::Integer(2)),
        ("kind", cbor::Value::text("board")),
        ("title", cbor::Value::text(title)),
        ("tags", cbor::Value::Array(Vec::new())),
        ("updated_at", cbor::Value::Integer(updated_at)),
        ("entries", cbor::Value::Array(Vec::new())),
    ])
}

fn write_external(temp: &TempWorkspace, id: u64, title: &str, updated_at: i64) {
    fs::write(
        temp.path(&format!("boards/{id}.board")),
        cbor::encode(&external_board(title, updated_at)),
    )
    .expect("外部写画板");
}

#[test]
fn missing_index_is_rebuilt_and_written_back() {
    let temp = TempWorkspace::new("index-rebuild");
    temp.workspace
        .save_at(1, &text_document("甲", "第一张正文"), 100)
        .expect("存盘");
    temp.workspace
        .save_at(2, &text_document("乙", "第二张正文"), 200)
        .expect("存盘");
    assert!(temp.path("index.cbor").is_file(), "保存后应该已经写了索引");
    fs::remove_file(temp.path("index.cbor")).expect("删索引");

    let reopened = Workspace::open(&temp.root).expect("重开工作区");
    let boards = reopened.list_boards().expect("列表");
    assert_eq!(boards.len(), 2);
    assert_eq!(boards[0].title, "乙");
    assert_eq!(boards[0].updated_at, 200);
    assert_eq!(boards[1].title, "甲");
    assert!(temp.path("index.cbor").is_file(), "重建后要把索引导回磁盘");

    let index =
        cbor::decode(&fs::read(temp.path("index.cbor")).expect("读索引")).expect("解码索引");
    assert!(
        index
            .get("format")
            .and_then(cbor::Value::as_integer)
            .is_some()
    );
    let entries = index
        .get("boards")
        .and_then(cbor::Value::as_array)
        .expect("索引有目录");
    assert_eq!(entries.len(), 2);
}

#[test]
fn corrupt_index_is_rebuilt_without_touching_boards() {
    let temp = TempWorkspace::new("index-corrupt");
    temp.workspace
        .save_at(1, &text_document("甲", "正文还在"), 100)
        .expect("存盘");
    fs::write(temp.path("index.cbor"), b"not cbor at all").expect("写坏索引");

    let reopened = Workspace::open(&temp.root).expect("重开工作区");
    let boards = reopened.list_boards().expect("列表");
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].title, "甲");
    // 画板内容没被动过。
    assert_eq!(reopened.load(1).expect("读画板").text_of(1), "正文还在");
    // 索引已经重写且能解码。
    assert!(cbor::decode(&fs::read(temp.path("index.cbor")).expect("读索引")).is_ok());
}

#[test]
fn external_board_changes_are_detected_on_reopen() {
    let temp = TempWorkspace::new("index-external");
    temp.workspace
        .save_at(1, &text_document("甲", "旧正文"), 100)
        .expect("存盘");
    assert_eq!(temp.workspace.list_boards().expect("列表").len(), 1);

    // 外部改了一张已有画板：标题与 updated_at 都变，文件长度也变。
    write_external(&temp, 1, "外部改过的", 999);
    let reopened = Workspace::open(&temp.root).expect("重开工作区");
    let boards = reopened.list_boards().expect("列表");
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].title, "外部改过的");
    assert_eq!(boards[0].updated_at, 999);

    // 外部新增一张。
    write_external(&temp, 2, "外部新增", 1_000);
    let reopened = Workspace::open(&temp.root).expect("重开工作区");
    let boards = reopened.list_boards().expect("列表");
    assert_eq!(boards.len(), 2);
    assert_eq!(boards[0].title, "外部新增");

    // 外部删掉一张。
    fs::remove_file(temp.path("boards/1.board")).expect("外部删画板");
    let reopened = Workspace::open(&temp.root).expect("重开工作区");
    let boards = reopened.list_boards().expect("列表");
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].title, "外部新增");
    assert!(
        reopened.load(2).expect("读画板").board.entries.is_empty(),
        "内容仍然可读"
    );
}

#[test]
fn saving_updates_the_index_incrementally() {
    let temp = TempWorkspace::new("index-save");
    let mut document = text_document("甲", "旧内容");
    temp.workspace.save_at(1, &document, 100).expect("存盘");
    assert_eq!(temp.workspace.search("旧内容").expect("搜索").len(), 1);

    document.board.title = "乙".to_owned();
    if let Some(Body::Text(block)) = document.body_mut(1) {
        block.text = "新内容".to_owned();
    }
    temp.workspace.save_at(1, &document, 200).expect("再存盘");

    // 同一个工作区实例，不重新打开也能看到新值。
    let boards = temp.workspace.list_boards().expect("列表");
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].title, "乙");
    assert_eq!(boards[0].updated_at, 200);
    assert_eq!(temp.workspace.search("新内容").expect("搜索").len(), 1);
    assert!(temp.workspace.search("旧内容").expect("搜索").is_empty());
    let (id, loaded) = temp.workspace.load_latest().expect("最近").expect("有画板");
    assert_eq!(id, 1);
    assert_eq!(loaded.board.title, "乙");
}

#[test]
fn deleting_a_board_updates_the_index() {
    let temp = TempWorkspace::new("index-delete");
    temp.workspace
        .save_at(1, &text_document("要删的", "删掉的正文"), 100)
        .expect("存盘");
    temp.workspace
        .save_at(2, &text_document("留下的", "留下的正文"), 200)
        .expect("存盘");
    assert_eq!(temp.workspace.list_boards().expect("列表").len(), 2);

    temp.workspace.delete_board(1).expect("删画板");
    let boards = temp.workspace.list_boards().expect("列表");
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].title, "留下的");
    assert!(
        temp.workspace
            .search("删掉的正文")
            .expect("搜索")
            .is_empty()
    );

    // 重新打开：索引与磁盘一致，不需要再补回已删的画板。
    let reopened = Workspace::open(&temp.root).expect("重开工作区");
    let boards = reopened.list_boards().expect("列表");
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].title, "留下的");
}

#[test]
fn corrupt_board_file_does_not_break_the_index() {
    let temp = TempWorkspace::new("index-corrupt-board");
    temp.workspace
        .save_at(1, &text_document("好的", "正常正文"), 100)
        .expect("存盘");
    fs::write(temp.path("boards/2.board"), b"this is not a board").expect("写坏画板");

    let reopened = Workspace::open(&temp.root).expect("重开工作区");
    assert_eq!(reopened.list_boards().expect("列表").len(), 1);
    // 再打开一次：坏画板有占位，集合仍然对得上，不该无限重建。
    let reopened = Workspace::open(&temp.root).expect("再重开");
    assert_eq!(reopened.list_boards().expect("列表").len(), 1);
    assert_eq!(reopened.search("正常正文").expect("搜索").len(), 1);
}

#[test]
fn rebuild_index_sees_external_changes_without_reopening() {
    let temp = TempWorkspace::new("index-force");
    temp.workspace
        .save_at(1, &text_document("甲", "第一张正文"), 100)
        .expect("存盘");
    assert_eq!(temp.workspace.list_boards().expect("列表").len(), 1);

    // 外部新增一张：没刷新前仍是旧索引。
    write_external(&temp, 2, "外部新增", 200);
    assert_eq!(
        temp.workspace.list_boards().expect("列表").len(),
        1,
        "没刷新前应该还是旧索引"
    );
    assert_eq!(temp.workspace.rebuild_index().expect("强制重建"), 2);
    let boards = temp.workspace.list_boards().expect("列表");
    assert_eq!(boards.len(), 2);
    assert_eq!(boards[0].title, "外部新增");
    let (id, _) = temp.workspace.load_latest().expect("最近").expect("有画板");
    assert_eq!(id, 2);

    // 外部改掉同一张：再重建能看到新标题与时间。
    write_external(&temp, 2, "外部改过", 300);
    assert_eq!(temp.workspace.rebuild_index().expect("强制重建"), 2);
    let boards = temp.workspace.list_boards().expect("列表");
    assert_eq!(boards[0].title, "外部改过");
    assert_eq!(boards[0].updated_at, 300);

    // 外部删掉一张：重建后不再返回它。
    fs::remove_file(temp.path("boards/1.board")).expect("外部删画板");
    assert_eq!(temp.workspace.rebuild_index().expect("强制重建"), 1);
    let boards = temp.workspace.list_boards().expect("列表");
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].title, "外部改过");
    assert!(
        temp.workspace
            .search("第一张正文")
            .expect("搜索")
            .is_empty()
    );
}

#[test]
fn rebuild_index_does_not_fail_on_a_corrupt_index_file() {
    let temp = TempWorkspace::new("index-force-corrupt");
    temp.workspace
        .save_at(1, &text_document("甲", "正文"), 100)
        .expect("存盘");
    fs::write(temp.path("index.cbor"), b"garbage").expect("写坏索引");

    let count = temp.workspace.rebuild_index().expect("索引坏了也能重建");
    assert_eq!(count, 1);
    assert_eq!(temp.workspace.list_boards().expect("列表").len(), 1);
    // 重建后索引文件重新可解码。
    assert!(cbor::decode(&fs::read(temp.path("index.cbor")).expect("读索引")).is_ok());

    // 重建不碰画板内容。
    assert_eq!(temp.workspace.load(1).expect("读画板").text_of(1), "正文");
}
