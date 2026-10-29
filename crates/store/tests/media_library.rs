//! 资源库：索引里那张媒体表。谁在用、用了几处、没人用了要不要消失。

mod support;

use notecevia_core::board::{Body, Entry, Geometry, ImageBlock};
use notecevia_core::{BlockKind, Document};
use notecevia_store::Workspace;

use support::TempWorkspace;

/// 一张放了一张图的笔记。返回存进去的哈希。
fn save_with_image(temp: &TempWorkspace, id: u64, bytes: &[u8], name: &str) -> String {
    let hash = temp.workspace.put_media(name, bytes).expect("放进媒体池");
    let mut document = Document::new("带图的笔记");
    document.board.entries.push(Entry::new(
        1,
        BlockKind::Image,
        Geometry::new(40.0, 40.0, 240.0, 160.0),
    ));
    document.bodies.insert(
        1,
        Body::Image(ImageBlock {
            media: hash.clone(),
            name: name.to_owned(),
            ..Default::default()
        }),
    );
    temp.workspace.save(id, &document).expect("存盘");
    hash
}

#[test]
fn a_used_image_shows_up_in_the_library() {
    let temp = TempWorkspace::new("media-library-used");
    let hash = save_with_image(&temp, 1, b"png-bytes", "截图.png");

    let media = temp.workspace.list_media().expect("列资源库");

    assert_eq!(media.len(), 1);
    assert_eq!(media[0].hash, hash);
    assert_eq!(media[0].name, "截图.png");
    assert_eq!(media[0].kind, BlockKind::Image);
    assert_eq!(media[0].bytes, 9, "大小来自媒体池里那份文件");
    assert_eq!(media[0].boards, 1);
}

#[test]
fn media_nobody_uses_leaves_the_library() {
    let temp = TempWorkspace::new("media-library-dropped");
    let hash = save_with_image(&temp, 1, b"png-bytes", "截图.png");

    // 把图从笔记里去掉再存一次：内容还在媒体池里，但资源库不该再列它。
    temp.workspace
        .save(1, &Document::new("空笔记"))
        .expect("存盘");

    assert!(temp.workspace.list_media().expect("列资源库").is_empty());
    assert!(
        temp.workspace.media_bytes(&hash).is_ok(),
        "媒体文件不回收：可能还有别的画板在用"
    );
}

#[test]
fn deleting_a_board_removes_its_media_from_the_library() {
    let temp = TempWorkspace::new("media-library-delete");
    save_with_image(&temp, 1, b"png-bytes", "截图.png");

    temp.workspace.delete_board(1).expect("删掉");

    assert!(temp.workspace.list_media().expect("列资源库").is_empty());
}

#[test]
fn the_same_file_used_by_two_boards_counts_twice() {
    let temp = TempWorkspace::new("media-library-shared");
    let hash = save_with_image(&temp, 1, b"png-bytes", "截图.png");
    // 第二张笔记引用同一个哈希：盘上只有一份文件。
    let mut document = Document::new("另一张");
    document.board.entries.push(Entry::new(
        1,
        BlockKind::Image,
        Geometry::new(0.0, 0.0, 100.0, 100.0),
    ));
    document.bodies.insert(
        1,
        Body::Image(ImageBlock {
            media: hash.clone(),
            name: "截图.png".to_owned(),
            ..Default::default()
        }),
    );
    temp.workspace.save(2, &document).expect("存盘");

    let media = temp.workspace.list_media().expect("列资源库");

    assert_eq!(media.len(), 1, "同一份内容只有一条");
    assert_eq!(media[0].boards, 2, "两张画板都在用");

    // 删掉一张，列表里还在，只是少了一处引用。
    temp.workspace.delete_board(2).expect("删掉一张");
    let media = temp.workspace.list_media().expect("再列一次");
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].boards, 1);
}

#[test]
fn a_rebuilt_index_still_lists_the_media() {
    let temp = TempWorkspace::new("media-library-rebuild");
    save_with_image(&temp, 1, b"png-bytes", "截图.png");

    temp.workspace.rebuild_index().expect("重建索引");

    let media = temp.workspace.list_media().expect("列资源库");
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].name, "截图.png");
    assert_eq!(media[0].boards, 1);
}

#[test]
fn a_fresh_window_sees_the_media_after_reopening() {
    let temp = TempWorkspace::new("media-library-reopen");
    save_with_image(&temp, 1, b"png-bytes", "截图.png");

    let reopened = Workspace::open(&temp.root).expect("重开工作区");

    let media = reopened.list_media().expect("列资源库");
    assert_eq!(media.len(), 1, "索引里存着媒体表，重开就还在");
}
