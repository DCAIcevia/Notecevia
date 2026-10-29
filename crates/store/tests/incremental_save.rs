//! 增量保存：只重写改动过的块，其余块文件保持原字节。
//!
//! 这里断言的是文件字节不动，不只是内容相等：save_changed 的意义就在于省掉
//! 那些没改块的写盘与 fsync，字节稳定才能说明真的没写。

mod support;

use std::fs;

use notecevia_core::board::{BlockKind, Body, Document, Geometry};

use support::{TempWorkspace, files_with_extension};

fn three_block_document() -> (Document, Vec<u64>) {
    let mut document = Document::new("三块画板");
    let mut ids = Vec::new();
    for index in 0..3u64 {
        let id = document.add_block(
            BlockKind::Text,
            Geometry::new(0.0, index as f32 * 40.0, 200.0, 30.0),
        );
        if let Some(Body::Text(block)) = document.body_mut(id) {
            block.text = format!("第 {id} 块");
        }
        ids.push(id);
    }
    (document, ids)
}

fn block_bytes(temp: &TempWorkspace, id: u64) -> Vec<u8> {
    fs::read(temp.path(&format!("blocks/1-{id}.block"))).expect("读块文件")
}

#[test]
fn save_changed_only_rewrites_changed_blocks() {
    let temp = TempWorkspace::new("incremental");
    let (mut document, ids) = three_block_document();
    temp.workspace.save_at(1, &document, 100).expect("全量保存");
    let before: Vec<Vec<u8>> = ids.iter().map(|id| block_bytes(&temp, *id)).collect();

    // 改第一块的文字，再把第二块挪个位置（目录项变了，正文没变）。
    if let Some(Body::Text(block)) = document.body_mut(ids[0]) {
        block.text = "改过的第一块".to_owned();
    }
    if let Some(entry) = document.entry_mut(ids[1]) {
        entry.geometry.x = 42.0;
    }
    temp.workspace
        .save_changed(1, &document, &[ids[0]])
        .expect("增量保存");

    let after: Vec<Vec<u8>> = ids.iter().map(|id| block_bytes(&temp, *id)).collect();
    assert_ne!(before[0], after[0], "改过的块应该重写");
    assert_eq!(before[1], after[1], "没改的块文件字节不该变");
    assert_eq!(before[2], after[2], "没改的块文件字节不该变");
    // 目录项是整份重写的，挪位置要能读回来。
    assert_eq!(temp.workspace.load(1).expect("读盘"), document);

    // changed 里有、文档里没有的编号直接跳过，不报错也不写东西。
    temp.workspace
        .save_changed(1, &document, &[999])
        .expect("未知编号跳过");
    let after_unknown: Vec<Vec<u8>> = ids.iter().map(|id| block_bytes(&temp, *id)).collect();
    assert_eq!(after, after_unknown, "未知编号不该引起块重写");
}

#[test]
fn deleted_block_goes_to_trash_on_incremental_save() {
    let temp = TempWorkspace::new("incremental-delete");
    let mut document = Document::new("删块画板");
    let keep = document.add_block(BlockKind::Text, Geometry::new(0.0, 0.0, 100.0, 30.0));
    let dropped = document.add_block(BlockKind::Text, Geometry::new(0.0, 40.0, 100.0, 30.0));
    if let Some(Body::Text(block)) = document.body_mut(keep) {
        block.text = "留着的块".to_owned();
    }
    if let Some(Body::Text(block)) = document.body_mut(dropped) {
        block.text = "要删的块".to_owned();
    }
    temp.workspace.save_at(1, &document, 100).expect("全量保存");
    let dropped_path = format!("blocks/1-{dropped}.block");
    assert!(temp.path(&dropped_path).exists());

    document.remove_block(dropped);
    // 删除的块不需要出现在 changed 里：孤儿回收会处理它。
    temp.workspace
        .save_changed(1, &document, &[])
        .expect("增量保存");

    assert!(
        !temp.path(&dropped_path).exists(),
        "被删的块文件应该离开 blocks/"
    );
    assert_eq!(
        files_with_extension(&temp.path(".trash"), "block").len(),
        1,
        "被删的块应该进回收站，而不是直接删"
    );
    let reloaded = temp.workspace.load(1).expect("读盘");
    assert_eq!(reloaded, document);
    assert_eq!(reloaded.board.entries.len(), 1);
    assert!(reloaded.body(dropped).is_none());
    assert_eq!(reloaded.text_of(keep), "留着的块");
}
