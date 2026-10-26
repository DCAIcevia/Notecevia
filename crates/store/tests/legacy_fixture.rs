//! 格式 1 的旧样例必须还能读：把仓库里的 fixtures/0.1.0-alpha.1 拷到临时目录再打开。
//!
//! 仓库里的样例只读：测试不写它，读完还要再看一眼格式号还是 1。

mod support;

use std::fs;
use std::path::{Path, PathBuf};

use notecevia_core::board::{BlockKind, Body, Geometry};
use notecevia_store::cbor;

use support::{TempWorkspace, copy_dir};

/// 样例放在仓库根目录，从 crate 目录往上两级。
fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/0.1.0-alpha.1")
}

fn board_format(path: &Path) -> i64 {
    let bytes = fs::read(path).expect("读画板文件");
    let value = cbor::decode(&bytes).expect("解码画板");
    value
        .get("format")
        .and_then(cbor::Value::as_integer)
        .expect("画板有格式号")
}

#[test]
fn format_one_fixture_still_loads() {
    let source = fixture_dir();
    assert!(
        source.join("boards/1.board").is_file(),
        "样例应该在仓库里：{}",
        source.display()
    );
    assert_eq!(board_format(&source.join("boards/1.board")), 1);

    let temp = TempWorkspace::new("legacy-fixture");
    copy_dir(&source, &temp.root).expect("拷样例到临时目录");

    let document = temp.workspace.load(1).expect("读格式 1 样例");
    assert_eq!(document.board.title, "格式样例 0.1.0-alpha.1");
    assert!(document.board.tags.is_empty());
    assert_eq!(document.board.entries.len(), 2);

    let first = &document.board.entries[0];
    assert_eq!(first.id, 1);
    assert_eq!(first.kind, BlockKind::Text);
    assert_eq!(first.geometry, Geometry::new(80.0, 80.0, 520.0, 60.0));
    assert_eq!((first.z, first.locked, first.visible), (0, false, true));

    let second = &document.board.entries[1];
    assert_eq!(second.id, 2);
    assert_eq!(second.geometry.x, 80.0);
    assert_eq!(second.geometry.y, 200.0);
    assert_eq!(second.geometry.width, 420.0);
    assert_eq!(second.geometry.height, 90.0);
    assert!((second.geometry.rotation - 0.12).abs() < 1e-6);

    let Some(Body::Text(first_text)) = document.body(1) else {
        panic!("第一块应该是文本块");
    };
    assert_eq!(
        first_text.text,
        "这张画板是 0.1.0-alpha.1 的格式样例：每次对外发布归档一份，用来测迁移。"
    );
    assert_eq!(first_text.size, Some(20.0));
    assert!(first_text.font.is_none());
    assert!(first_text.color.is_none());
    assert!(first_text.background.is_none());

    let Some(Body::Text(second_text)) = document.body(2) else {
        panic!("第二块应该是文本块");
    };
    assert_eq!(second_text.font.as_deref(), Some("Microsoft YaHei"));
    assert_eq!(second_text.size, Some(16.0));
    assert_eq!(second_text.color, Some([0x2c, 0x6c, 0xc8]));
    assert_eq!(second_text.background, Some([0xff, 0xee, 0xaa]));
}

#[test]
fn loading_an_old_board_and_saving_writes_the_new_format() {
    let source = fixture_dir();
    let temp = TempWorkspace::new("legacy-rewrite");
    copy_dir(&source, &temp.root).expect("拷样例");

    let document = temp.workspace.load(1).expect("读旧样例");
    temp.workspace
        .save_at(1, &document, 1_767_225_600)
        .expect("写回");
    assert_eq!(board_format(&temp.path("boards/1.board")), 2);
    assert_eq!(temp.workspace.load(1).expect("再读"), document);

    // 仓库里的样例只读，格式号没被测试改过。
    assert_eq!(board_format(&source.join("boards/1.board")), 1);
}
