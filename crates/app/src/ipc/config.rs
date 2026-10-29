//! 应用配置的读取与写入。配置在程序的配置目录里，与工作区无关。

use std::path::PathBuf;

use tauri::Manager;

use crate::config::AppConfig;
use crate::ipc::error::IpcError;

/// 配置文件名。整个目录由 Tauri 按平台解析，这里只管文件名。
const CONFIG_FILE: &str = "config.json";

fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, IpcError> {
    app.path()
        .app_config_dir()
        .map(|directory| directory.join(CONFIG_FILE))
        .map_err(|error| IpcError::internal(format!("取配置目录失败：{error}")))
}

/// 读到的配置与它来自哪里。
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfigDto {
    pub path: String,
    pub config: AppConfig,
}

/// 读应用配置。工作区没打开也能用。
#[tauri::command]
pub fn app_config(app: tauri::AppHandle) -> Result<AppConfigDto, IpcError> {
    let path = config_path(&app)?;
    Ok(AppConfigDto {
        config: AppConfig::load(&path),
        path: path.to_string_lossy().into_owned(),
    })
}

/// 写应用配置。写失败就是失败，不退回内存状态。
#[tauri::command]
pub fn save_app_config(app: tauri::AppHandle, config: AppConfig) -> Result<(), IpcError> {
    let path = config_path(&app)?;
    config
        .save(&path)
        .map_err(|error| IpcError::new("io", format!("写配置失败：{error}")))
}
