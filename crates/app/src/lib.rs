//! Notecevia 的窗口壳：起 Tauri 应用，把 Web 前端接到 Rust 内核上。

#![deny(rustdoc::broken_intra_doc_links)]
#![forbid(unsafe_code)]

pub mod config;
pub mod ipc;
pub mod watchdog;

use ipc::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 装饰插件必须在创建窗口之前注册。
        .plugin(tauri_plugin_decoration::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .manage(watchdog::Heartbeat::new())
        .setup(|app| {
            // 前端脚本挂死时窗口要能自己关掉：六十秒没有心跳就退出。
            watchdog::spawn(app.handle().clone(), std::time::Duration::from_secs(60));

            // 窗口先隐藏等自绘装饰激活；前端起不来也要露出窗口，别留个看不见的进程。
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(15));
                if let Some(window) = tauri::Manager::get_webview_window(&handle, "main")
                    && !window.is_visible().unwrap_or(false)
                {
                    let _ = window.show();
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::decoration::activate_and_show,
            ipc::health::heartbeat,
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
            ipc::library::create_page,
            ipc::library::shelve_volume,
            ipc::library::unshelve_volume,
            ipc::library::create_shelf_with,
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
