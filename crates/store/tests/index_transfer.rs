//! 列表、搜索、删除与容器：首页与导出走的都是这些。

mod support;

use notecevia_core::board::{BlockKind, Body, Document, Geometry};

use support::{TempWorkspace, files_with_extension};

fn text_document(title: &str, body: &str) -> Document {
    let mut document = Document::new(title);
    let id = document.add_block(BlockKind::Text, Geometry::new(0.0, 0.0, 100.0, 40.0));
    if let Some(Body::Text(block)) = document.body_mut(id) {
        block.text = body.to_owned();
    }
    document
}

#[test]
fn boards_are_listed_newest_first_with_tags_and_counts() {
    let temp = TempWorkspace::new("list");
    let mut old = text_document("旧画板", "归档内容");
    old.board.add_tag("归档");
    old.add_block(BlockKind::Table, Geometry::new(0.0, 50.0, 100.0, 60.0));
    let new = text_document("新画板", "最近改的");
    temp.workspace.save_at(1, &old, 100).expect("存旧");
    temp.workspace.save_at(2, &new, 300).expect("存新");

    let boards = temp.workspace.list_boards().expect("列表");
    assert_eq!(boards.len(), 2);
    assert_eq!(boards[0].id, 2);
    assert_eq!(boards[0].title, "新画板");
    assert_eq!(boards[0].updated_at, 300);
    assert_eq!(boards[0].entry_count, 1);
    assert_eq!(boards[1].id, 1);
    assert_eq!(boards[1].title, "旧画板");
    assert_eq!(boards[1].tags, vec!["归档".to_owned()]);
    assert_eq!(boards[1].entry_count, 2);

    let (id, document) = temp.workspace.load_latest().expect("最近").expect("有画板");
    assert_eq!(id, 2);
    assert_eq!(document, new);
}

#[test]
fn boards_with_the_same_time_are_ordered_by_number() {
    let temp = TempWorkspace::new("list-tie");
    temp.workspace
        .save_at(3, &text_document("甲", "甲"), 100)
        .expect("存盘");
    temp.workspace
        .save_at(8, &text_document("乙", "乙"), 100)
        .expect("存盘");
    let boards = temp.workspace.list_boards().expect("列表");
    assert_eq!(
        boards.iter().map(|board| board.id).collect::<Vec<_>>(),
        vec![8, 3]
    );
    let (id, _) = temp.workspace.load_latest().expect("最近").expect("有画板");
    assert_eq!(id, 8);
}

#[test]
fn search_matches_title_tag_and_text() {
    let temp = TempWorkspace::new("search");
    let mut meeting = text_document("会议记录", "下周要交付登录页和导出功能");
    meeting.board.add_tag("工作");
    let idea = text_document("灵感", "海边与风");
    temp.workspace.save_at(1, &meeting, 200).expect("存盘");
    temp.workspace.save_at(2, &idea, 100).expect("存盘");

    let hits = temp.workspace.search("交付").expect("搜索正文");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].board_id, 1);
    assert_eq!(hits[0].title, "会议记录");
    assert!(hits[0].snippet.contains("交付"));

    let hits = temp.workspace.search("工作").expect("搜索标签");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].board_id, 1);
    assert!(hits[0].snippet.contains("工作"));

    let hits = temp.workspace.search("灵感").expect("搜索标题");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].board_id, 2);

    assert!(
        temp.workspace
            .search("不存在的词")
            .expect("搜索")
            .is_empty()
    );
    assert!(temp.workspace.search("   ").expect("空查询").is_empty());
}

#[test]
fn search_ignores_ascii_case() {
    let temp = TempWorkspace::new("search-case");
    temp.workspace
        .save_at(1, &text_document("Export", "HTML Board"), 1)
        .expect("存盘");
    let hits = temp.workspace.search("export").expect("小写搜索");
    assert_eq!(hits.len(), 1);
    let hits = temp.workspace.search("html").expect("命中正文");
    assert_eq!(hits.len(), 1);
    assert!(hits[0].snippet.contains("HTML"));
}

#[test]
fn deleting_a_board_moves_it_and_its_blocks_to_trash() {
    let temp = TempWorkspace::new("delete");
    let image = temp
        .workspace
        .put_media("photo.png", b"png bytes")
        .expect("放媒体");
    let mut document = text_document("要删的画板", "正文");
    let image_id = document.add_block(BlockKind::Image, Geometry::new(0.0, 0.0, 10.0, 10.0));
    if let Some(Body::Image(block)) = document.body_mut(image_id) {
        block.media = image.clone();
    }
    temp.workspace.save_at(1, &document, 100).expect("存盘");

    temp.workspace.delete_board(1).expect("删画板");

    assert!(!temp.path("boards/1.board").exists());
    assert!(!temp.path("blocks/1-1.block").exists());
    assert!(!temp.path(&format!("blocks/1-{image_id}.block")).exists());
    assert!(temp.workspace.list_boards().expect("列表").is_empty());
    assert!(
        temp.workspace
            .load(1)
            .expect("读空画板")
            .board
            .entries
            .is_empty()
    );

    assert_eq!(files_with_extension(&temp.path(".trash"), "board").len(), 1);
    assert_eq!(files_with_extension(&temp.path(".trash"), "block").len(), 2);
    // 媒体不回收：同一份图可能还被别的画板引用。
    assert!(temp.workspace.media_path(&image).exists());
}

#[test]
fn container_roundtrip_keeps_everything() {
    let source = TempWorkspace::new("container-source");
    let image = source
        .workspace
        .put_media("photo.png", b"png bytes")
        .expect("放媒体");
    let mut document = text_document("带走这张", "正文与 <符号>");
    document.board.add_tag("带走");
    let image_id = document.add_block(BlockKind::Image, Geometry::new(10.0, 50.0, 80.0, 60.0));
    if let Some(Body::Image(block)) = document.body_mut(image_id) {
        block.media = image.clone();
        block.name = "photo.png".to_owned();
    }
    if let Some(entry) = document.entry_mut(image_id) {
        entry.locked = true;
    }
    let table_id = document.add_block(BlockKind::Table, Geometry::new(0.0, 120.0, 100.0, 60.0));
    if let Some(Body::Table(block)) = document.body_mut(table_id) {
        block.set_cell(0, 0, "格");
    }
    source.workspace.save_at(1, &document, 500).expect("存盘");

    let container = source.path("board.export");
    source
        .workspace
        .export_container(1, &container)
        .expect("打包");

    let target = TempWorkspace::new("container-target");
    let imported = target.workspace.import_container(&container).expect("导入");
    assert_eq!(imported, 1);
    assert_eq!(target.workspace.load(imported).expect("读盘"), document);
    assert_eq!(
        target.workspace.media_bytes(&image).expect("媒体"),
        b"png bytes"
    );

    let boards = target.workspace.list_boards().expect("列表");
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].title, "带走这张");
    assert_eq!(boards[0].updated_at, 500);
    assert_eq!(boards[0].tags, vec!["带走".to_owned()]);
    assert_eq!(boards[0].entry_count, 3);

    // 再导一次是一张新画板，不覆盖已有内容。
    let again = target
        .workspace
        .import_container(&container)
        .expect("再导入");
    assert_eq!(again, 2);
    assert_eq!(target.workspace.load(again).expect("读盘"), document);
    assert_eq!(target.workspace.list_boards().expect("列表").len(), 2);
}
