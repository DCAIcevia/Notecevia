//! 旧样例必须还能读：把仓库里的 fixtures 拷到临时目录再打开。
//!
//! 格式 1 的兼容测试现在走迁移路径（migrate.rs 显式补字段）；格式 2 的样例
//! 用来对照，证明当前格式不会再被迁移动一次。仓库里的样例只读。

mod support;

use std::fs;
use std::path::{Path, PathBuf};

use notecevia_core::board::{BlockKind, Body, Geometry};
use notecevia_store::cbor;

use support::{TempWorkspace, copy_dir};

/// 样例放在仓库根目录，从 crate 目录往上两级。
fn fixture_dir(version: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(version)
}

fn decode_board(path: &Path) -> cbor::Value {
    let bytes = fs::read(path).expect("读画板文件");
    cbor::decode(&bytes).expect("解码画板")
}

fn board_format(path: &Path) -> i64 {
    decode_board(path)
        .get("format")
        .and_then(cbor::Value::as_integer)
        .expect("画板有格式号")
}

#[test]
fn format_one_fixture_still_loads() {
    let source = fixture_dir("0.1.0-alpha.1");
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
fn format_one_goes_through_migration_and_saves_as_format_two() {
    let source = fixture_dir("0.1.0-alpha.1");
    let temp = TempWorkspace::new("legacy-rewrite");
    copy_dir(&source, &temp.root).expect("拷样例");

    let document = temp.workspace.load(1).expect("读旧样例");
    // 迁移补默认值：格式 1 没有 tags/updated_at/locked/visible。
    for entry in &document.board.entries {
        assert!(!entry.locked);
        assert!(entry.visible);
    }
    let boards = temp.workspace.list_boards().expect("列表");
    assert_eq!(boards.len(), 1);
    assert_eq!(boards[0].updated_at, 0, "格式 1 没有 updated_at，迁移补 0");

    temp.workspace
        .save_at(1, &document, 1_767_225_600)
        .expect("写回");
    assert_eq!(board_format(&temp.path("boards/1.board")), 2);

    let saved = decode_board(&temp.path("boards/1.board"));
    assert_eq!(
        saved.get("updated_at").and_then(cbor::Value::as_integer),
        Some(1_767_225_600)
    );
    assert_eq!(
        saved
            .get("tags")
            .and_then(cbor::Value::as_array)
            .map(|tags| tags.len()),
        Some(0)
    );
    let entries = saved
        .get("entries")
        .and_then(cbor::Value::as_array)
        .expect("有目录");
    for entry in entries {
        assert_eq!(
            entry.get("locked").and_then(cbor::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            entry.get("visible").and_then(cbor::Value::as_bool),
            Some(true)
        );
    }

    assert_eq!(temp.workspace.load(1).expect("再读"), document);
    // 仓库里的样例只读，格式号没被测试改过。
    assert_eq!(board_format(&source.join("boards/1.board")), 1);
}

#[test]
fn format_two_fixture_loads_and_saves_back_unchanged() {
    let source = fixture_dir("0.2.0-alpha.1");
    assert!(
        source.join("boards/1.board").is_file(),
        "0.2.0 样例应该在仓库里：{}",
        source.display()
    );
    assert_eq!(board_format(&source.join("boards/1.board")), 2);

    let temp = TempWorkspace::new("fixture-2");
    copy_dir(&source, &temp.root).expect("拷样例到临时目录");

    let document = temp.workspace.load(1).expect("读格式 2 样例");
    assert_eq!(document.board.title, "格式样例 0.2.0-alpha.1");
    assert_eq!(
        document.board.tags,
        vec!["样例".to_owned(), "格式".to_owned()]
    );
    assert_eq!(document.board.entries.len(), 7);

    // 格式 2 不该被迁移再动一次：按样例自己的时间写回，字节应与样例一致。
    temp.workspace
        .save_at(1, &document, 1_767_225_600)
        .expect("写回");
    for file in ["boards/1.board", "blocks/1-1.block", "blocks/1-7.block"] {
        assert_eq!(
            fs::read(temp.path(file)).expect("读临时文件"),
            fs::read(source.join(file)).expect("读样例"),
            "{file} 写回后应与样例逐字节一致"
        );
    }
}
