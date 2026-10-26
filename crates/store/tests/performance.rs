//! 一万张画板的性能实测。常规 CI 不跑，需要时手动执行：
//!
//! cargo test -p notecevia-store --release --test performance -- --ignored --nocapture
//!
//! 生成的都是最小合法文件（画板 + 一个文本块），数据写在系统临时目录，跑完删掉。
//! 分两段看：没有索引的老工作区要先重建一次（一次性）；模拟真实启动时索引已存在，
//! 只校验与读索引，这才是日常路径。
//! 最后一段专门量增量保存：一千块的画板，全量 save_at 与只改一块的 save_changed。

mod support;

use std::fs;
use std::path::Path;
use std::time::Instant;

use notecevia_core::board::{BlockKind, Body, Document, Geometry};
use notecevia_store::{Workspace, cbor};

use support::TempWorkspace;

const BOARDS: u64 = 10_000;
/// 固定基准时间：样例的 updated_at 从这里往上加。
const BASE_TIME: i64 = 1_700_000_000;

fn write_board_files(root: &Path, id: u64) {
    let reference = format!("blocks/{id}-1.block");
    let board = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(2)),
        ("kind", cbor::Value::text("board")),
        ("title", cbor::Value::text(format!("画板 {id}"))),
        ("tags", cbor::Value::Array(vec![cbor::Value::text("批量")])),
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
    )
    .expect("写画板");

    let block = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(2)),
        ("block_type", cbor::Value::Integer(1)),
        ("id", cbor::Value::Integer(1)),
        (
            "text",
            cbor::Value::text(format!("第 {id} 张画板的性能测试内容")),
        ),
    ]);
    fs::write(
        root.join("blocks").join(format!("{id}-1.block")),
        cbor::encode(&block),
    )
    .expect("写块");
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

fn report(label: &str, elapsed: std::time::Duration, budget_ms: u128) {
    let millis = elapsed.as_millis();
    let verdict = if millis <= budget_ms {
        "预算内"
    } else {
        "超预算"
    };
    println!("{label}: {millis} ms（{elapsed:?}，预算 {budget_ms} ms，{verdict}）");
}

#[test]
#[ignore = "一万张画板，只在需要测性能时手动跑"]
fn ten_thousand_boards_are_measured() {
    let temp = TempWorkspace::new("performance");
    let generation = Instant::now();
    for id in 1..=BOARDS {
        write_board_files(&temp.root, id);
    }
    println!(
        "生成 {BOARDS} 张画板（{} 个文件）耗时：{:?}",
        BOARDS * 2,
        generation.elapsed()
    );

    // 一、老工作区没有索引：第一次列表要逐板读一遍，重建整份索引。
    let rebuild_started = Instant::now();
    let boards = temp.workspace.list_boards().expect("重建索引并列表");
    let rebuild_time = rebuild_started.elapsed();
    assert_eq!(boards.len(), usize::try_from(BOARDS).expect("能放下"));
    let index_size = fs::metadata(temp.path("index.cbor"))
        .expect("重建后应该有索引")
        .len();
    println!("首次重建索引（含列表）：{rebuild_time:?}，index.cbor {index_size} 字节");

    // 二、模拟真实启动：索引已存在，只校验与读索引。
    let open_started = Instant::now();
    let workspace = Workspace::open(&temp.root).expect("打开工作区");
    let open_time = open_started.elapsed();

    let latest_started = Instant::now();
    let (latest_id, _) = workspace
        .load_latest()
        .expect("读最近画板")
        .expect("有画板");
    let latest_time = latest_started.elapsed();
    assert_eq!(latest_id, BOARDS);

    let list_started = Instant::now();
    let boards = workspace.list_boards().expect("列表");
    let list_time = list_started.elapsed();
    assert_eq!(boards.len(), usize::try_from(BOARDS).expect("能放下"));

    let search_started = Instant::now();
    let hits = workspace.search("性能测试").expect("搜索");
    let search_time = search_started.elapsed();
    assert_eq!(hits.len(), usize::try_from(BOARDS).expect("能放下"));

    // 三、保存一次：写块、写画板、回收孤儿，再整份原子重写索引。
    let document = workspace.load(1).expect("读画板");
    let save_started = Instant::now();
    workspace
        .save_at(1, &document, BASE_TIME + 1_000_000)
        .expect("保存");
    let save_time = save_started.elapsed();

    let (bytes, files) = directory_size(&temp.root);
    println!(
        "目录体积：{bytes} 字节（{} MiB），{files} 个文件",
        bytes / (1024 * 1024)
    );
    println!("Workspace::open：{open_time:?}");
    report(
        "启动 load_latest（含打开与索引校验）",
        open_time + latest_time,
        500,
    );
    report("list_boards（一万张，读索引）", list_time, 1_500);
    report("search（一万张，命中一万条，读索引）", search_time, 100);
    println!("save_at（含索引重写，单次）：{save_time:?}");

    // 四、一千块画板的增量保存：全量写一千个块 vs 只重写改动的块。
    let incremental = TempWorkspace::new("performance-save-changed");
    let mut document = Document::new("一千块画板");
    for index in 0..1_000u64 {
        let id = document.add_block(
            BlockKind::Text,
            Geometry::new(0.0, index as f32 * 30.0, 240.0, 24.0),
        );
        if let Some(Body::Text(block)) = document.body_mut(id) {
            block.text = format!("第 {id} 块");
        }
    }
    incremental
        .workspace
        .save_at(1, &document, BASE_TIME)
        .expect("初次全量保存");

    let full_started = Instant::now();
    incremental
        .workspace
        .save_at(1, &document, BASE_TIME + 1)
        .expect("全量保存");
    let full_time = full_started.elapsed();

    if let Some(Body::Text(block)) = document.body_mut(1) {
        block.text = "只改一块".to_owned();
    }
    let changed_started = Instant::now();
    incremental
        .workspace
        .save_changed(1, &document, &[1])
        .expect("增量保存");
    let changed_time = changed_started.elapsed();

    println!("1000 块画板全量 save_at：{full_time:?}");
    println!("1000 块画板 save_changed（只改 1 块）：{changed_time:?}");
}
