//! 快速造一批合法画板，给冷启动与容量实测用。
//!
//! 用法：
//! cargo run -p notecevia-store --release --example make_boards -- <输出目录> [数量]
//!
//! 直接写 boards/ 与 blocks/ 文件，不走 save_at——逐张保存每次都要重写索引，
//! 一万张会退化成 O(n²)。全部写完只调一次 rebuild_index。
//! 时间戳由固定基准加画板编号得到，内容固定，重复运行写出的字节相同。
//! 输出目录里原有的画板也会被一起索引；要干净的实测数据请用空目录。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use notecevia_store::{Workspace, cbor};

const DEFAULT_COUNT: u64 = 10_000;
/// 固定基准时间：画板 i 的 updated_at 是 BASE_TIME + i，和系统时间无关。
const BASE_TIME: i64 = 1_700_000_000;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let Some(target) = args.next().map(PathBuf::from) else {
        eprintln!("用法：cargo run -p notecevia-store --example make_boards -- <输出目录> [数量]");
        return ExitCode::FAILURE;
    };
    let count = match args.next() {
        Some(raw) => match raw.to_str().and_then(|text| text.parse::<u64>().ok()) {
            Some(count) if count > 0 => count,
            _ => {
                eprintln!("数量必须是正整数：{}", raw.to_string_lossy());
                return ExitCode::FAILURE;
            }
        },
        None => DEFAULT_COUNT,
    };

    match run(&target, count) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("生成画板失败：{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(target: &Path, count: u64) -> Result<(), notecevia_store::Error> {
    let workspace = Workspace::open(target)?;
    let root = workspace.root().to_path_buf();

    let generation = Instant::now();
    for id in 1..=count {
        write_board(&root, id)?;
        write_block(&root, id)?;
    }
    let generation_time = generation.elapsed();

    let index_started = Instant::now();
    let indexed = workspace.rebuild_index()?;
    let index_time = index_started.elapsed();

    let (bytes, files) = directory_size(&root);
    println!(
        "写入 {count} 张画板（{} 个文件）：{generation_time:?}",
        count.saturating_mul(2)
    );
    println!("重建索引：{index_time:?}，共 {indexed} 条");
    println!(
        "目录体积：{bytes} 字节（{} MiB），{files} 个文件",
        bytes / (1024 * 1024)
    );
    Ok(())
}

fn write_board(root: &Path, id: u64) -> Result<(), notecevia_store::Error> {
    let reference = format!("blocks/{id}-1.block");
    let board = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(2)),
        ("kind", cbor::Value::text("board")),
        ("title", cbor::Value::text(format!("画板 {id}"))),
        ("tags", cbor::Value::Array(Vec::new())),
        (
            "updated_at",
            cbor::Value::Integer(BASE_TIME + i64::try_from(id).unwrap_or(i64::MAX)),
        ),
        (
            "entries",
            cbor::Value::Array(vec![cbor::Value::map(vec![
                ("id", cbor::Value::Integer(1)),
                ("block_type", cbor::Value::Integer(1)),
                ("block_ref", cbor::Value::text(reference)),
                ("x", cbor::Value::Float(80.0)),
                ("y", cbor::Value::Float(80.0)),
                ("w", cbor::Value::Float(320.0)),
                ("h", cbor::Value::Float(120.0)),
                ("rotation", cbor::Value::Float(0.0)),
                ("z_index", cbor::Value::Integer(0)),
                ("locked", cbor::Value::Bool(false)),
                ("visible", cbor::Value::Bool(true)),
            ])]),
        ),
    ]);
    fs::write(
        root.join("boards").join(format!("{id}.board")),
        cbor::encode(&board),
    )?;
    Ok(())
}

fn write_block(root: &Path, id: u64) -> Result<(), notecevia_store::Error> {
    let block = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(2)),
        ("block_type", cbor::Value::Integer(1)),
        ("id", cbor::Value::Integer(1)),
        ("text", cbor::Value::text(format!("第 {id} 张画板的正文"))),
    ]);
    fs::write(
        root.join("blocks").join(format!("{id}-1.block")),
        cbor::encode(&block),
    )?;
    Ok(())
}

/// 目录体积与文件数。跳过回收站：它不该出现在正常数据里。
fn directory_size(root: &Path) -> (u64, u64) {
    let mut bytes = 0;
    let mut files = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                if path.file_name().and_then(|name| name.to_str()) != Some(".trash") {
                    stack.push(path);
                }
            } else if let Ok(metadata) = entry.metadata() {
                bytes += metadata.len();
                files += 1;
            }
        }
    }
    (bytes, files)
}
