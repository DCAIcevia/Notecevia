//! 落盘格式、媒体池、原子写与回收站。这些都是坏了会丢用户数据的地方。

mod support;

use std::fs;
use std::path::Path;

use notecevia_core::board::{BlockKind, Body, Document, Geometry, ImageFit, ShapeKind, TableBlock};
use notecevia_store::{Error, cbor};

use support::{TempWorkspace, files_with_extension};

const PNG: &[u8] = &[
    0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x01, 0x02, 0x03,
];
const VIDEO: &[u8] = b"pretend this is a video";
const AUDIO: &[u8] = b"pretend this is audio";

fn decode_file(path: &Path) -> cbor::Value {
    let bytes = fs::read(path).expect("读文件");
    cbor::decode(&bytes).expect("解码")
}

fn integer(value: &cbor::Value, key: &str) -> i64 {
    value
        .get(key)
        .and_then(cbor::Value::as_integer)
        .expect("有整数字段")
}

fn text(value: &cbor::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(cbor::Value::as_text)
        .expect("有文本字段")
        .to_owned()
}

fn entry_with(board: &cbor::Value, id: i64) -> &cbor::Value {
    board
        .get("entries")
        .and_then(cbor::Value::as_array)
        .expect("有目录")
        .iter()
        .find(|entry| entry.get("id").and_then(cbor::Value::as_integer) == Some(id))
        .expect("有这一条目录项")
}

/// 七类块各一块，样式、层级、锁定、隐藏都填上。
fn rich_document(image: &str, video: &str, audio: &str) -> Document {
    let mut document = Document::new("年度总结");
    document.board.add_tag("工作");
    document.board.add_tag("2026");

    let text_id = document.add_block(BlockKind::Text, Geometry::new(10.0, 20.0, 200.0, 80.0));
    if let Some(Body::Text(block)) = document.body_mut(text_id) {
        block.text = "第一段中文\n第二行 & <tag>".to_owned();
        block.font = Some("SimSun".to_owned());
        block.size = Some(18.5);
        block.color = Some([0x20, 0x30, 0x40]);
        block.background = Some([240, 230, 140]);
    }

    let image_id = document.add_block(BlockKind::Image, Geometry::new(220.0, 20.0, 120.0, 90.0));
    if let Some(entry) = document.entry_mut(image_id) {
        entry.geometry.rotation = 0.5;
        entry.locked = true;
        entry.z = 2;
    }
    if let Some(Body::Image(block)) = document.body_mut(image_id) {
        block.media = image.to_owned();
        block.name = "photo.png".to_owned();
        block.fit = ImageFit::Contain;
    }

    let video_id = document.add_block(BlockKind::Video, Geometry::new(350.0, 20.0, 160.0, 90.0));
    if let Some(entry) = document.entry_mut(video_id) {
        entry.visible = false;
    }
    if let Some(Body::Media(block)) = document.body_mut(video_id) {
        block.media = video.to_owned();
        block.name = "clip.mp4".to_owned();
        block.bytes = 1_048_576;
    }

    let audio_id = document.add_block(BlockKind::Audio, Geometry::new(10.0, 120.0, 180.0, 60.0));
    if let Some(Body::Media(block)) = document.body_mut(audio_id) {
        block.media = audio.to_owned();
        block.name = "voice.m4a".to_owned();
        block.bytes = 4096;
    }

    let table_id = document.add_block(BlockKind::Table, Geometry::new(220.0, 130.0, 180.0, 90.0));
    if let Some(entry) = document.entry_mut(table_id) {
        entry.z = 3;
    }
    if let Some(Body::Table(block)) = document.body_mut(table_id) {
        *block = TableBlock::new(2, 3);
        block.set_cell(0, 0, "项目");
        block.set_cell(0, 1, "状态");
        block.set_cell(0, 2, "备注");
        block.set_cell(1, 0, "导出");
        block.set_cell(1, 1, "进行中");
        block.set_cell(1, 2, "含中文 & 符号");
    }

    let shape_id = document.add_block(BlockKind::Shape, Geometry::new(10.0, 230.0, 120.0, 80.0));
    if let Some(entry) = document.entry_mut(shape_id) {
        entry.locked = true;
    }
    if let Some(Body::Shape(block)) = document.body_mut(shape_id) {
        block.shape = ShapeKind::Ellipse;
        block.fill = Some([0x33, 0x66, 0x99]);
        block.stroke = Some([0x11, 0x22, 0x33]);
        block.stroke_width = 3.5;
    }

    let highlight_id =
        document.add_block(BlockKind::Highlight, Geometry::new(0.0, 300.0, 200.0, 30.0));
    if let Some(entry) = document.entry_mut(highlight_id) {
        entry.z = -1;
    }
    if let Some(Body::Highlight(block)) = document.body_mut(highlight_id) {
        block.color = [0xff, 0xe0, 0x66];
        block.opacity = 0.4;
    }

    document
}

fn rich_workspace(name: &str) -> (TempWorkspace, Document) {
    let temp = TempWorkspace::new(name);
    let image = temp.workspace.put_media("photo.png", PNG).expect("放图片");
    let video = temp.workspace.put_media("clip.mp4", VIDEO).expect("放视频");
    let audio = temp
        .workspace
        .put_media("voice.m4a", AUDIO)
        .expect("放音频");
    (temp, rich_document(&image, &video, &audio))
}

#[test]
fn every_block_kind_survives_a_roundtrip() {
    let (temp, document) = rich_workspace("roundtrip");
    temp.workspace
        .save_at(1, &document, 1_700_000_000)
        .expect("存盘");
    assert_eq!(temp.workspace.load(1).expect("读盘"), document);
}

#[test]
fn board_file_carries_format_two_fields() {
    let (temp, document) = rich_workspace("board-fields");
    temp.workspace
        .save_at(7, &document, 1_700_000_000)
        .expect("存盘");

    let board = decode_file(&temp.path("boards/7.board"));
    assert_eq!(integer(&board, "format"), 2);
    assert_eq!(text(&board, "kind"), "board");
    assert_eq!(text(&board, "title"), "年度总结");
    assert_eq!(integer(&board, "updated_at"), 1_700_000_000);
    let tags: Vec<String> = board
        .get("tags")
        .and_then(cbor::Value::as_array)
        .expect("有标签")
        .iter()
        .filter_map(cbor::Value::as_text)
        .map(str::to_owned)
        .collect();
    assert_eq!(tags, vec!["工作".to_owned(), "2026".to_owned()]);
    assert_eq!(
        board
            .get("entries")
            .and_then(cbor::Value::as_array)
            .expect("有目录")
            .len(),
        7
    );

    let image_entry = entry_with(&board, 2);
    assert_eq!(integer(image_entry, "block_type"), 2);
    assert_eq!(text(image_entry, "block_ref"), "blocks/7-2.block");
    assert_eq!(integer(image_entry, "z_index"), 2);
    assert_eq!(
        image_entry.get("locked").and_then(cbor::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        image_entry.get("visible").and_then(cbor::Value::as_bool),
        Some(true)
    );
    let video_entry = entry_with(&board, 3);
    assert_eq!(
        video_entry.get("visible").and_then(cbor::Value::as_bool),
        Some(false)
    );
}

#[test]
fn block_files_carry_type_specific_fields() {
    let (temp, document) = rich_workspace("block-fields");
    let image_hash = document
        .body(2)
        .and_then(|body| match body {
            Body::Image(image) => Some(image.media.clone()),
            _ => None,
        })
        .expect("有图片块");
    let video_hash = document
        .body(3)
        .and_then(|body| match body {
            Body::Media(media) => Some(media.media.clone()),
            _ => None,
        })
        .expect("有视频块");
    temp.workspace.save_at(7, &document, 1).expect("存盘");

    let text = decode_file(&temp.path("blocks/7-1.block"));
    assert_eq!(integer(&text, "format"), 2);
    assert_eq!(integer(&text, "block_type"), 1);
    assert_eq!(integer(&text, "id"), 1);
    assert_eq!(text_of(&text, "text"), "第一段中文\n第二行 & <tag>");
    assert_eq!(text_of(&text, "font"), "SimSun");
    assert_eq!(float_of(&text, "size") as f32, 18.5);
    assert_eq!(text_of(&text, "color"), "#203040");
    assert_eq!(text_of(&text, "background"), "#f0e68c");

    let image = decode_file(&temp.path("blocks/7-2.block"));
    assert_eq!(integer(&image, "block_type"), 2);
    assert_eq!(text_of(&image, "media"), image_hash);
    assert_eq!(text_of(&image, "name"), "photo.png");
    assert_eq!(text_of(&image, "fit"), "contain");

    let video = decode_file(&temp.path("blocks/7-3.block"));
    assert_eq!(integer(&video, "block_type"), 3);
    assert_eq!(text_of(&video, "media"), video_hash);
    assert_eq!(integer(&video, "bytes"), 1_048_576);

    let audio = decode_file(&temp.path("blocks/7-4.block"));
    assert_eq!(integer(&audio, "block_type"), 4);
    assert_eq!(integer(&audio, "bytes"), 4096);

    let table = decode_file(&temp.path("blocks/7-5.block"));
    assert_eq!(integer(&table, "block_type"), 5);
    assert_eq!(integer(&table, "rows"), 2);
    assert_eq!(integer(&table, "cols"), 3);
    let cells: Vec<String> = table
        .get("cells")
        .and_then(cbor::Value::as_array)
        .expect("有单元格")
        .iter()
        .filter_map(cbor::Value::as_text)
        .map(str::to_owned)
        .collect();
    assert_eq!(cells.len(), 6);
    assert_eq!(cells[4], "进行中");

    let shape = decode_file(&temp.path("blocks/7-6.block"));
    assert_eq!(integer(&shape, "block_type"), 6);
    assert_eq!(text_of(&shape, "shape"), "ellipse");
    assert_eq!(text_of(&shape, "fill"), "#336699");
    assert_eq!(text_of(&shape, "stroke"), "#112233");
    assert_eq!(float_of(&shape, "stroke_width"), 3.5);

    let highlight = decode_file(&temp.path("blocks/7-7.block"));
    assert_eq!(integer(&highlight, "block_type"), 7);
    assert_eq!(text_of(&highlight, "color"), "#ffe066");
    assert!((float_of(&highlight, "opacity") as f32 - 0.4).abs() < 1e-6);
}

fn text_of(value: &cbor::Value, key: &str) -> String {
    text(value, key)
}

fn float_of(value: &cbor::Value, key: &str) -> f64 {
    value
        .get(key)
        .and_then(cbor::Value::as_float)
        .expect("有浮点字段")
}

#[test]
fn identical_media_is_stored_once() {
    let temp = TempWorkspace::new("media");
    let first = temp
        .workspace
        .put_media("a.png", b"same bytes")
        .expect("放媒体");
    let second = temp
        .workspace
        .put_media("b.png", b"same bytes")
        .expect("放媒体");
    assert_eq!(first, second);
    assert_eq!(first.len(), 64);
    assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));

    let path = temp.workspace.media_path(&first);
    assert!(path.exists());
    assert_eq!(
        temp.workspace.media_bytes(&first).expect("读媒体"),
        b"same bytes"
    );
    assert_eq!(files_with_extension(&temp.path("media"), "tmp").len(), 0);

    let other = temp
        .workspace
        .put_media("a.png", b"other bytes")
        .expect("放媒体");
    assert_ne!(first, other);
    let files = fs::read_dir(temp.path("media"))
        .expect("列媒体目录")
        .count();
    assert_eq!(files, 2);
}

#[test]
fn media_hash_matches_the_standard_vector() {
    let temp = TempWorkspace::new("media-vector");
    assert_eq!(
        temp.workspace.put_media("abc.txt", b"abc").expect("放媒体"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn media_names_that_are_not_hashes_are_refused() {
    let temp = TempWorkspace::new("media-name");
    // 不合法的哈希一律落到媒体目录本身，不会被拼成工作区外的路径。
    let media_dir = temp.path("media");
    assert_eq!(temp.workspace.media_path("../../secret"), media_dir);
    assert_eq!(temp.workspace.media_path("no-hash"), media_dir);
    assert!(matches!(
        temp.workspace.media_bytes("../../secret"),
        Err(Error::BadMediaName(_))
    ));
}

#[test]
fn saving_leaves_no_temporary_files_and_recover_cleans_strays() {
    let (temp, document) = rich_workspace("atomic");
    temp.workspace.save_at(1, &document, 100).expect("存盘");
    assert!(files_with_extension(&temp.root, "tmp").is_empty());
    assert_eq!(temp.workspace.recover().expect("清临时文件"), 0);

    fs::write(temp.path("boards/9.tmp"), b"half written").expect("放临时文件");
    fs::write(temp.path("blocks/1-9.tmp"), b"half written").expect("放临时文件");
    assert_eq!(temp.workspace.recover().expect("清临时文件"), 2);
    assert!(files_with_extension(&temp.root, "tmp").is_empty());
}

#[test]
fn load_follows_block_ref_and_refuses_escapes() {
    let temp = TempWorkspace::new("block-ref");

    // 工作区外的块文件：即使真的存在，也不能被读进来。
    let outside = temp.outside_path("block");
    let secret = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(2)),
        ("block_type", cbor::Value::Integer(1)),
        ("id", cbor::Value::Integer(2)),
        ("text", cbor::Value::text("工作区外的内容")),
    ]);
    fs::write(&outside, cbor::encode(&secret)).expect("写工作区外的块");
    let outside_ref = format!(
        "../{}",
        outside.file_name().expect("有文件名").to_string_lossy()
    );

    let board = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(2)),
        ("kind", cbor::Value::text("board")),
        ("title", cbor::Value::text("地址测试")),
        ("tags", cbor::Value::Array(Vec::new())),
        ("updated_at", cbor::Value::Integer(7)),
        (
            "entries",
            cbor::Value::Array(vec![
                test_entry(1, 1, "blocks/custom-1.block", 0.0),
                test_entry(2, 1, &outside_ref, 100.0),
                test_entry(3, 1, "C:/Windows/evil.block", 200.0),
            ]),
        ),
    ]);
    fs::write(temp.path("boards/5.board"), cbor::encode(&board)).expect("写画板");

    let custom = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(2)),
        ("block_type", cbor::Value::Integer(1)),
        ("id", cbor::Value::Integer(1)),
        ("text", cbor::Value::text("按 block_ref 找到的正文")),
    ]);
    fs::write(temp.path("blocks/custom-1.block"), cbor::encode(&custom)).expect("写块");

    let document = temp.workspace.load(5).expect("读画板");
    assert_eq!(document.board.entries.len(), 3);
    assert_eq!(document.text_of(1), "按 block_ref 找到的正文");
    assert!(document.body(2).is_none());
    assert!(document.body(3).is_none());

    let _ = fs::remove_file(&outside);
}

fn test_entry(id: i64, block_type: i64, reference: &str, x: f64) -> cbor::Value {
    cbor::Value::map(vec![
        ("id", cbor::Value::Integer(id)),
        ("block_type", cbor::Value::Integer(block_type)),
        ("block_ref", cbor::Value::text(reference)),
        ("x", cbor::Value::Float(x)),
        ("y", cbor::Value::Float(0.0)),
        ("w", cbor::Value::Float(10.0)),
        ("h", cbor::Value::Float(10.0)),
        ("rotation", cbor::Value::Float(0.0)),
        ("z_index", cbor::Value::Integer(0)),
        ("locked", cbor::Value::Bool(false)),
        ("visible", cbor::Value::Bool(true)),
    ])
}

#[test]
fn missing_block_ref_falls_back_to_the_numbered_path() {
    let temp = TempWorkspace::new("fallback");
    let board = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(2)),
        ("kind", cbor::Value::text("board")),
        ("title", cbor::Value::text("旧文件")),
        ("entries", cbor::Value::Array(vec![legacy_entry(4)])),
    ]);
    fs::write(temp.path("boards/6.board"), cbor::encode(&board)).expect("写画板");
    let block = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(1)),
        ("block_type", cbor::Value::Integer(1)),
        ("id", cbor::Value::Integer(4)),
        ("text", cbor::Value::text("没有 block_ref 的旧块")),
    ]);
    fs::write(temp.path("blocks/6-4.block"), cbor::encode(&block)).expect("写块");

    let document = temp.workspace.load(6).expect("读画板");
    assert_eq!(document.text_of(4), "没有 block_ref 的旧块");
}

fn legacy_entry(id: i64) -> cbor::Value {
    cbor::Value::map(vec![
        ("id", cbor::Value::Integer(id)),
        ("block_type", cbor::Value::Integer(1)),
        ("x", cbor::Value::Float(0.0)),
        ("y", cbor::Value::Float(0.0)),
        ("w", cbor::Value::Float(10.0)),
        ("h", cbor::Value::Float(10.0)),
        ("z_index", cbor::Value::Integer(0)),
    ])
}

#[test]
fn unknown_block_types_and_fields_do_not_break_loading() {
    let temp = TempWorkspace::new("unknown");
    let board = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(2)),
        ("kind", cbor::Value::text("board")),
        ("title", cbor::Value::text("未来的块")),
        (
            "entries",
            cbor::Value::Array(vec![
                test_entry(1, 1, "blocks/1-1.block", 0.0),
                test_entry(2, 99, "blocks/1-2.block", 50.0),
            ]),
        ),
    ]);
    fs::write(temp.path("boards/1.board"), cbor::encode(&board)).expect("写画板");
    let block = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(2)),
        ("block_type", cbor::Value::Integer(1)),
        ("id", cbor::Value::Integer(1)),
        ("text", cbor::Value::text("看得见的正文")),
        // 将来加的字段：读的时候不认识就不管，不能因此丢掉整块。
        ("future_field", cbor::Value::text("以后再说")),
    ]);
    fs::write(temp.path("blocks/1-1.block"), cbor::encode(&block)).expect("写块");

    let document = temp.workspace.load(1).expect("读画板");
    assert_eq!(document.board.entries.len(), 1);
    assert_eq!(document.text_of(1), "看得见的正文");
}

#[test]
fn newer_format_is_refused() {
    let temp = TempWorkspace::new("newer");
    let board = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(3)),
        ("title", cbor::Value::text("未来的画板")),
        ("entries", cbor::Value::Array(Vec::new())),
    ]);
    fs::write(temp.path("boards/1.board"), cbor::encode(&board)).expect("写画板");
    assert!(matches!(
        temp.workspace.load(1),
        Err(Error::NewerFormat {
            found: 3,
            supported: 2
        })
    ));
}

#[test]
fn format_one_files_still_load() {
    let temp = TempWorkspace::new("legacy");
    let board = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(1)),
        ("kind", cbor::Value::text("board")),
        ("title", cbor::Value::text("旧画板")),
        ("entries", cbor::Value::Array(vec![legacy_entry(1)])),
    ]);
    fs::write(temp.path("boards/1.board"), cbor::encode(&board)).expect("写画板");
    let block = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(1)),
        ("block_type", cbor::Value::Integer(1)),
        ("id", cbor::Value::Integer(1)),
        ("text", cbor::Value::text("旧格式正文")),
    ]);
    fs::write(temp.path("blocks/1-1.block"), cbor::encode(&block)).expect("写块");

    let document = temp.workspace.load(1).expect("读旧画板");
    assert_eq!(document.board.title, "旧画板");
    assert!(document.board.tags.is_empty());
    let entry = &document.board.entries[0];
    assert!(!entry.locked);
    assert!(entry.visible);
    assert_eq!(document.text_of(1), "旧格式正文");
}

#[test]
fn saving_moves_orphan_blocks_to_trash_and_keeps_media() {
    let temp = TempWorkspace::new("orphan");
    let image = temp.workspace.put_media("photo.png", PNG).expect("放媒体");
    let mut document = Document::new("回收测试");
    let text = document.add_block(BlockKind::Text, Geometry::new(0.0, 0.0, 100.0, 40.0));
    let image_id = document.add_block(BlockKind::Image, Geometry::new(0.0, 60.0, 100.0, 80.0));
    if let Some(Body::Image(block)) = document.body_mut(image_id) {
        block.media = image.clone();
        block.name = "photo.png".to_owned();
    }
    temp.workspace.save_at(1, &document, 100).expect("存盘");
    assert!(temp.path(&format!("blocks/1-{text}.block")).exists());
    assert!(temp.path(&format!("blocks/1-{image_id}.block")).exists());

    document.remove_block(text);
    document.remove_block(image_id);
    temp.workspace.save_at(1, &document, 100).expect("再存盘");

    assert!(!temp.path(&format!("blocks/1-{text}.block")).exists());
    assert!(!temp.path(&format!("blocks/1-{image_id}.block")).exists());
    assert!(
        temp.path(&format!(".trash/100-blocks/1-{text}.block"))
            .exists(),
        "孤儿块应该进回收站，而不是直接删"
    );
    // 媒体不回收：同一份图可能还被别处引用。
    assert!(temp.workspace.media_path(&image).exists());
    assert!(
        temp.workspace
            .load(1)
            .expect("读盘")
            .board
            .entries
            .is_empty()
    );
}

#[test]
fn unknown_fields_survive_a_load_save_roundtrip() {
    let temp = TempWorkspace::new("unknown-fields");
    // 手工构造：画板、目录项、块正文都带上本程序不认识的字段。
    let board = cbor::Value::map_with_extras(
        vec![
            ("format", cbor::Value::Integer(2)),
            ("kind", cbor::Value::text("board")),
            ("title", cbor::Value::text("未来格式")),
            ("tags", cbor::Value::Array(vec![cbor::Value::text("保留")])),
            ("updated_at", cbor::Value::Integer(111)),
            (
                "entries",
                cbor::Value::Array(vec![cbor::Value::map_with_extras(
                    vec![
                        ("id", cbor::Value::Integer(1)),
                        ("block_type", cbor::Value::Integer(1)),
                        ("block_ref", cbor::Value::text("blocks/1-1.block")),
                        ("x", cbor::Value::Float(10.0)),
                        ("y", cbor::Value::Float(20.0)),
                        ("w", cbor::Value::Float(30.0)),
                        ("h", cbor::Value::Float(40.0)),
                        ("rotation", cbor::Value::Float(0.0)),
                        ("z_index", cbor::Value::Integer(0)),
                        ("locked", cbor::Value::Bool(false)),
                        ("visible", cbor::Value::Bool(true)),
                    ],
                    vec![
                        (cbor::Value::text("future_entry"), cbor::Value::Integer(7)),
                        (
                            cbor::Value::text("future_entry_text"),
                            cbor::Value::text("留着我"),
                        ),
                    ],
                )]),
            ),
        ],
        vec![(cbor::Value::text("future_board"), cbor::Value::Bool(true))],
    );
    fs::write(temp.path("boards/1.board"), cbor::encode(&board)).expect("写画板");

    let block = cbor::Value::map_with_extras(
        vec![
            ("format", cbor::Value::Integer(2)),
            ("block_type", cbor::Value::Integer(1)),
            ("id", cbor::Value::Integer(1)),
            ("text", cbor::Value::text("正文")),
        ],
        vec![
            (
                cbor::Value::text("future_block"),
                cbor::Value::text("块里的未来字段"),
            ),
            (cbor::Value::text("future_number"), cbor::Value::Float(1.5)),
        ],
    );
    fs::write(temp.path("blocks/1-1.block"), cbor::encode(&block)).expect("写块");

    let document = temp.workspace.load(1).expect("读画板");
    assert_eq!(document.text_of(1), "正文");
    temp.workspace.save_at(1, &document, 111).expect("写回");

    let saved_board = decode_file(&temp.path("boards/1.board"));
    assert_eq!(
        saved_board
            .get("future_board")
            .and_then(cbor::Value::as_bool),
        Some(true)
    );
    let entry = entry_with(&saved_board, 1);
    assert_eq!(
        entry.get("future_entry").and_then(cbor::Value::as_integer),
        Some(7)
    );
    assert_eq!(
        entry
            .get("future_entry_text")
            .and_then(cbor::Value::as_text),
        Some("留着我")
    );

    let saved_block = decode_file(&temp.path("blocks/1-1.block"));
    assert_eq!(
        saved_block
            .get("future_block")
            .and_then(cbor::Value::as_text),
        Some("块里的未来字段")
    );
    assert_eq!(
        saved_block
            .get("future_number")
            .and_then(cbor::Value::as_float),
        Some(1.5)
    );
    assert_eq!(
        saved_block.get("text").and_then(cbor::Value::as_text),
        Some("正文")
    );

    // 改过的字段仍然按新值写：未知字段不会盖掉已知字段。
    let mut document = document;
    if let Some(Body::Text(block)) = document.body_mut(1) {
        block.text = "改过的正文".to_owned();
    }
    temp.workspace.save_at(1, &document, 111).expect("再写回");
    let saved_block = decode_file(&temp.path("blocks/1-1.block"));
    assert_eq!(
        saved_block.get("text").and_then(cbor::Value::as_text),
        Some("改过的正文")
    );
    assert_eq!(
        saved_block
            .get("future_block")
            .and_then(cbor::Value::as_text),
        Some("块里的未来字段"),
        "已知字段更新时，未知字段还要在"
    );
}

#[test]
fn thumbnails_are_optional_and_follow_the_board_into_trash() {
    let temp = TempWorkspace::new("thumbnail");
    assert!(
        temp.workspace
            .thumbnail_bytes(1)
            .expect("读缩略图")
            .is_none()
    );
    temp.workspace
        .save_thumbnail(1, b"png bytes")
        .expect("存缩略图");
    assert_eq!(
        temp.workspace.thumbnail_bytes(1).expect("读缩略图"),
        Some(b"png bytes".to_vec())
    );
    assert!(temp.path("thumbnails/1.png").is_file());

    temp.workspace
        .save_at(1, &Document::new("画板"), 100)
        .expect("存画板");
    temp.workspace.delete_board(1).expect("删画板");
    assert!(
        !temp.path("thumbnails/1.png").exists(),
        "缩略图应该跟着画板进回收站"
    );
    assert_eq!(files_with_extension(&temp.path(".trash"), "png").len(), 1);
    assert!(
        temp.workspace
            .thumbnail_bytes(1)
            .expect("读缩略图")
            .is_none()
    );
}
