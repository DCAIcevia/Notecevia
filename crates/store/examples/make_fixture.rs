//! 生成 fixtures/0.2.0-alpha.1：一张含全部七类块、媒体与标签的样例工作区。
//!
//! 用法：
//! cargo run -p notecevia-store --example make_fixture -- fixtures/0.2.0-alpha.1
//!
//! 时间戳与内容都是固定的，重复运行写出的字节相同。目录已存在时只覆盖同名文件；
//! 生成器内容改过之后，请在空目录重跑（或先手动删掉旧样例），免得留下没人引用的文件。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use notecevia_core::board::{BlockKind, Body, Document, Geometry, ImageFit, ShapeKind};
use notecevia_store::Workspace;

/// 2026-01-01T00:00:00Z。固定时间戳，样例才能重复生成出同一份字节。
const UPDATED_AT: i64 = 1_767_225_600;

/// 16×16 的纯色 PNG（#2c6cc8）。手写字节，生成样例时不碰图片解码依赖。
const PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x10, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x91, 0x68,
    0x36, 0x00, 0x00, 0x00, 0x16, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xd0, 0xc9, 0x39, 0x41,
    0x12, 0x62, 0x18, 0xd5, 0x30, 0xaa, 0x61, 0xf8, 0x6a, 0x00, 0x00, 0x86, 0x96, 0x60, 0x10, 0x68,
    0xc1, 0x1d, 0xed, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// 视频与音频放占位字节：样例用来测格式，不提供可播放内容。
const VIDEO: &[u8] = b"fixture placeholder for a video block";
const AUDIO: &[u8] = b"fixture placeholder for an audio block";

fn main() -> ExitCode {
    let target = std::env::args_os().nth(1).map(PathBuf::from);
    let Some(target) = target else {
        eprintln!("用法：cargo run -p notecevia-store --example make_fixture -- <输出目录>");
        return ExitCode::FAILURE;
    };
    match write_fixture(&target) {
        Ok(()) => {
            println!("样例已写入 {}", target.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("生成样例失败：{error}");
            ExitCode::FAILURE
        }
    }
}

fn write_fixture(target: &Path) -> Result<(), notecevia_store::Error> {
    let workspace = Workspace::open(target)?;
    let png = workspace.put_media("sample.png", PNG)?;
    let video = workspace.put_media("sample.mp4", VIDEO)?;
    let audio = workspace.put_media("sample.m4a", AUDIO)?;
    let document = fixture_document(&png, &video, &audio);
    workspace.save_at(1, &document, UPDATED_AT)
}

fn fixture_document(png: &str, video: &str, audio: &str) -> Document {
    let mut document = Document::new("格式样例 0.2.0-alpha.1");
    document.board.add_tag("样例");
    document.board.add_tag("格式");

    let text = document.add_block(BlockKind::Text, Geometry::new(80.0, 80.0, 520.0, 64.0));
    if let Some(Body::Text(block)) = document.body_mut(text) {
        block.text =
            "这张画板是 0.2.0-alpha.1 的格式样例：七类块、媒体池与标签都在里面。".to_owned();
        block.font = Some("Microsoft YaHei".to_owned());
        block.size = Some(20.0);
        block.color = Some([0x1f, 0x23, 0x28]);
    }

    let image = document.add_block(BlockKind::Image, Geometry::new(80.0, 180.0, 240.0, 180.0));
    if let Some(entry) = document.entry_mut(image) {
        entry.locked = true;
        entry.z = 1;
    }
    if let Some(Body::Image(block)) = document.body_mut(image) {
        block.media = png.to_owned();
        block.name = "sample.png".to_owned();
        block.fit = ImageFit::Contain;
    }

    let video_id = document.add_block(BlockKind::Video, Geometry::new(350.0, 180.0, 250.0, 140.0));
    if let Some(entry) = document.entry_mut(video_id) {
        entry.z = 1;
    }
    if let Some(Body::Media(block)) = document.body_mut(video_id) {
        block.media = video.to_owned();
        block.name = "sample.mp4".to_owned();
        block.bytes = VIDEO.len() as u64;
    }

    let audio_id = document.add_block(BlockKind::Audio, Geometry::new(80.0, 400.0, 300.0, 60.0));
    if let Some(entry) = document.entry_mut(audio_id) {
        entry.z = 1;
    }
    if let Some(Body::Media(block)) = document.body_mut(audio_id) {
        block.media = audio.to_owned();
        block.name = "sample.m4a".to_owned();
        block.bytes = AUDIO.len() as u64;
    }

    let table_id = document.add_block(BlockKind::Table, Geometry::new(400.0, 340.0, 260.0, 150.0));
    if let Some(entry) = document.entry_mut(table_id) {
        entry.z = 1;
    }
    if let Some(Body::Table(block)) = document.body_mut(table_id) {
        *block = notecevia_core::board::TableBlock::new(2, 3);
        block.set_cell(0, 0, "项目");
        block.set_cell(0, 1, "状态");
        block.set_cell(0, 2, "负责人");
        block.set_cell(1, 0, "导出");
        block.set_cell(1, 1, "进行中");
        block.set_cell(1, 2, "store");
    }

    let shape_id = document.add_block(BlockKind::Shape, Geometry::new(80.0, 500.0, 200.0, 120.0));
    if let Some(entry) = document.entry_mut(shape_id) {
        entry.geometry.rotation = 0.12;
        entry.z = 3;
    }
    if let Some(Body::Shape(block)) = document.body_mut(shape_id) {
        block.shape = ShapeKind::Ellipse;
        block.fill = Some([0x2c, 0x6c, 0xc8]);
        block.stroke = Some([0x1f, 0x23, 0x28]);
        block.stroke_width = 3.5;
    }

    let highlight_id = document.add_block(
        BlockKind::Highlight,
        Geometry::new(300.0, 500.0, 260.0, 48.0),
    );
    if let Some(entry) = document.entry_mut(highlight_id) {
        entry.z = 0;
    }
    if let Some(Body::Highlight(block)) = document.body_mut(highlight_id) {
        block.color = [0xff, 0xe0, 0x66];
        block.opacity = 0.55;
    }

    if let Some(entry) = document.entry_mut(text) {
        entry.z = 5;
    }
    document
}
