//! 首页索引：重建、增量更新、删除、外部改动与损坏恢复。
//!
//! 索引只是缓存，画板文件才是事实：这些用例都同时检查内容没被索引影响。

mod support;

use std::fs;

use notecevia_core::board::{BlockKind, Body, Document, Geometry, TableBlock};
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
fn external_add_delete_and_rename_are_detected_on_reopen() {
    let temp = TempWorkspace::new("index-external");
    temp.workspace
        .save_at(1, &text_document("甲", "旧正文"), 100)
        .expect("存盘");
    assert_eq!(temp.workspace.list_boards().expect("列表").len(), 1);

    // 外部新增一张：文件数与目录时间都变，打开时能发现并重建。
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

    // 外部改名：原子改名会改目录时间，打开时同样能发现。
    fs::rename(temp.path("boards/2.board"), temp.path("boards/3.board")).expect("外部改名");
    let reopened = Workspace::open(&temp.root).expect("重开工作区");
    let boards = reopened.list_boards().expect("列表");
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].id, 3);
    assert_eq!(boards[0].title, "外部新增");
}

#[test]
fn in_place_overwrite_needs_rebuild_to_be_seen() {
    let temp = TempWorkspace::new("index-in-place");
    temp.workspace
        .save_at(1, &text_document("甲", "旧正文"), 100)
        .expect("存盘");
    assert_eq!(temp.workspace.list_boards().expect("列表")[0].title, "甲");

    // 原地覆盖写：文件数不变、目录时间不变，打开时发现不了（有意的取舍）。
    write_external(&temp, 1, "原地改的", 999);
    let reopened = Workspace::open(&temp.root).expect("重开工作区");
    let boards = reopened.list_boards().expect("列表");
    assert_eq!(boards.len(), 1);
    assert_eq!(
        boards[0].title, "甲",
        "原地覆盖写不改变目录指纹，索引不会自动重建"
    );
    assert_eq!(boards[0].updated_at, 100);

    // rebuild_index 能立刻刷新。
    assert_eq!(reopened.rebuild_index().expect("强制重建"), 1);
    let boards = reopened.list_boards().expect("列表");
    assert_eq!(boards[0].title, "原地改的");
    assert_eq!(boards[0].updated_at, 999);
    assert_eq!(reopened.load(1).expect("读画板").board.title, "原地改的");
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

/// 一张含表格与媒体的画板：表格里有独特词，图片与视频的文件名也是独特词。
fn rich_document(image_media: &str, video_media: &str) -> Document {
    let mut document = text_document("表格与媒体", "普通正文");
    let table = document.add_block(BlockKind::Table, Geometry::new(0.0, 100.0, 240.0, 120.0));
    if let Some(Body::Table(block)) = document.body_mut(table) {
        *block = TableBlock::new(2, 2);
        block.set_cell(0, 0, "季度目标");
        block.set_cell(1, 1, "完成一半");
    }
    let image = document.add_block(BlockKind::Image, Geometry::new(0.0, 240.0, 120.0, 120.0));
    if let Some(Body::Image(block)) = document.body_mut(image) {
        block.media = image_media.to_owned();
        block.name = "季度照片.png".to_owned();
    }
    let video = document.add_block(BlockKind::Video, Geometry::new(140.0, 240.0, 120.0, 120.0));
    if let Some(Body::Media(block)) = document.body_mut(video) {
        block.media = video_media.to_owned();
        block.name = "会议录像.mp4".to_owned();
        block.bytes = 4;
    }
    document
}

/// 把索引改成旧格式（格式号 1、texts 清空），文件指纹保持有效：
/// 这样只有格式号不对能触发重建，能证明不是靠指纹碰巧重建的。
fn downgrade_index(temp: &TempWorkspace) {
    let index =
        cbor::decode(&fs::read(temp.path("index.cbor")).expect("读索引")).expect("解码索引");
    let cbor::Value::Map(mut pairs) = index else {
        panic!("索引应该是映射");
    };
    for (key, value) in &mut pairs {
        match key {
            cbor::Value::Text(name) if name == "format" => *value = cbor::Value::Integer(1),
            cbor::Value::Text(name) if name == "boards" => {
                let cbor::Value::Array(boards) = value else {
                    continue;
                };
                for board in boards {
                    let cbor::Value::Map(board_pairs) = board else {
                        continue;
                    };
                    for (board_key, board_value) in board_pairs {
                        if matches!(board_key, cbor::Value::Text(name) if name == "texts") {
                            *board_value = cbor::Value::Array(Vec::new());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    fs::write(
        temp.path("index.cbor"),
        cbor::encode(&cbor::Value::Map(pairs)),
    )
    .expect("写旧索引");
}

#[test]
fn search_matches_table_cells_and_media_names() {
    let temp = TempWorkspace::new("index-search-rich");
    let image = temp
        .workspace
        .put_media("季度照片.png", b"png bytes")
        .expect("放图片");
    let video = temp
        .workspace
        .put_media("会议录像.mp4", b"video bytes")
        .expect("放视频");
    temp.workspace
        .save_at(1, &rich_document(&image, &video), 100)
        .expect("存盘");

    // 同一个工作区实例：save_at 的增量索引也要带上表格与文件名。
    for query in ["季度目标", "完成一半", "季度照片.png", "会议录像.mp4"] {
        let hits = temp.workspace.search(query).expect("搜索");
        assert_eq!(hits.len(), 1, "{query} 应该命中这张画板");
        assert!(
            hits[0].snippet.contains(query),
            "{query} 的 snippet 应包含命中的词：{:?}",
            hits[0].snippet
        );
    }
    // 普通正文仍然搜得到，标题与标签的优先级没有被改。
    assert_eq!(temp.workspace.search("普通正文").expect("搜索").len(), 1);
    assert!(
        temp.workspace
            .search("不存在的词")
            .expect("搜索")
            .is_empty()
    );

    // 重新打开走重建路径，结果一致。
    let reopened = Workspace::open(&temp.root).expect("重开工作区");
    for query in ["季度目标", "季度照片.png", "会议录像.mp4"] {
        let hits = reopened.search(query).expect("搜索");
        assert_eq!(hits.len(), 1, "{query} 重建索引后仍应命中");
    }
}

#[test]
fn old_index_format_is_rebuilt_automatically() {
    let temp = TempWorkspace::new("index-old-format");
    let image = temp
        .workspace
        .put_media("季度照片.png", b"png bytes")
        .expect("放图片");
    let video = temp
        .workspace
        .put_media("会议录像.mp4", b"video bytes")
        .expect("放视频");
    temp.workspace
        .save_at(1, &rich_document(&image, &video), 100)
        .expect("存盘");
    assert_eq!(temp.workspace.list_boards().expect("列表").len(), 1);

    downgrade_index(&temp);
    let reopened = Workspace::open(&temp.root).expect("重开工作区");
    let hits = reopened.search("季度目标").expect("搜索表格单元格");
    assert_eq!(hits.len(), 1, "低格式号的旧索引应该被丢弃并重建");
    assert!(hits[0].snippet.contains("季度目标"));

    let index =
        cbor::decode(&fs::read(temp.path("index.cbor")).expect("读索引")).expect("解码索引");
    assert_eq!(
        index.get("format").and_then(cbor::Value::as_integer),
        Some(4),
        "重建后应该写回当前索引格式号"
    );
}
