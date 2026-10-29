//! Notecevia 的窗口壳：起 Tauri 应用，把 Web 前端接到 Rust 内核上。

#![deny(rustdoc::broken_intra_doc_links)]
#![forbid(unsafe_code)]

/// 示例命令：确认前端能调到 Rust 侧。
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
