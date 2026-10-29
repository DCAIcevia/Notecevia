//! Notecevia 的窗口壳：起 Tauri 应用，把 Web 前端接到 Rust 内核上。

#![deny(rustdoc::broken_intra_doc_links)]
#![forbid(unsafe_code)]

pub mod config;
pub mod ipc;

use ipc::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            ipc::config::app_config,
            ipc::config::save_app_config,
            ipc::state::app_info,
            ipc::state::open_workspace,
            ipc::state::workspace_status,
            ipc::boards::list_boards,
            ipc::boards::search_boards,
            ipc::boards::rebuild_index,
            ipc::boards::list_media,
            ipc::boards::next_board_id,
            ipc::boards::open_board,
            ipc::boards::save_board,
            ipc::boards::create_board,
            ipc::boards::rename_board,
            ipc::boards::delete_board,
            ipc::media::read_media,
            ipc::media::write_media,
            ipc::media::read_thumbnail,
            ipc::media::save_thumbnail,
            ipc::library::list_volumes,
            ipc::library::load_volume,
            ipc::library::save_volume,
            ipc::library::create_volume,
            ipc::library::delete_volume,
            ipc::library::assign_page,
            ipc::library::unassign_page,
            ipc::library::move_page,
            ipc::library::move_page_in_volume,
            ipc::library::pages_in_volume,
            ipc::library::volume_of_page,
            ipc::library::list_shelves,
            ipc::library::load_shelf,
            ipc::library::save_shelf,
            ipc::library::create_shelf,
            ipc::library::delete_shelf,
            ipc::trash::list_trash,
            ipc::trash::restore_trash,
            ipc::trash::purge_trash_item,
            ipc::trash::empty_trash,
            ipc::transfer::export_container,
            ipc::transfer::export_html,
            ipc::transfer::import_container,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
