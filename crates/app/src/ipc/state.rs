//! 应用级状态：全局只持有一个工作区，命令都从这里拿。

use std::path::PathBuf;
use std::sync::Mutex;

use notecevia_store::{Workspace, workspace_root};
use serde::Serialize;

use super::error::IpcError;

/// 已打开的工作区信息。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    /// 工作区根目录的绝对路径。
    pub root: String,
    /// 这次打开时从回收站清掉的过期条目数。
    pub purged_on_open: usize,
}

/// 应用级状态。`Workspace` 里的索引缓存不是 `Sync`，所以整份放进锁里。
pub struct AppState {
    workspace: Mutex<Option<Workspace>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            workspace: Mutex::new(None),
        }
    }

    /// 打开工作区，成功时替换掉已经开着的那个。
    ///
    /// 打不开时旧的工作区保持不变：用户还能继续用已经打开的数据。
    pub fn open(&self, root: Option<PathBuf>) -> Result<WorkspaceStatus, IpcError> {
        let path = root.unwrap_or_else(workspace_root);
        let workspace = Workspace::open(path).map_err(IpcError::from)?;
        let status = WorkspaceStatus {
            root: workspace.root().display().to_string(),
            purged_on_open: workspace.purged_on_open(),
        };
        let mut guard = self.lock()?;
        *guard = Some(workspace);
        Ok(status)
    }

    /// 当前工作区信息；还没打开时为 `None`。
    pub fn status(&self) -> Result<Option<WorkspaceStatus>, IpcError> {
        let guard = self.lock()?;
        Ok(guard.as_ref().map(|workspace| WorkspaceStatus {
            root: workspace.root().display().to_string(),
            purged_on_open: workspace.purged_on_open(),
        }))
    }

    /// 在已打开的工作区上执行。没打开就报 `workspaceNotOpen`。
    pub fn with<T>(
        &self,
        run: impl FnOnce(&Workspace) -> Result<T, IpcError>,
    ) -> Result<T, IpcError> {
        let guard = self.lock()?;
        let workspace = guard.as_ref().ok_or_else(IpcError::workspace_not_open)?;
        run(workspace)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Option<Workspace>>, IpcError> {
        self.workspace
            .lock()
            .map_err(|_| IpcError::internal("工作区状态锁被 poisoned"))
    }
}

/// 应用与默认数据位置。前端启动时先问这个。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub default_workspace: String,
}

/// 打开工作区。`path` 省略时用程序目录下的 `NoteceviaData`。
#[tauri::command]
pub fn open_workspace(
    state: tauri::State<'_, AppState>,
    path: Option<String>,
) -> Result<WorkspaceStatus, IpcError> {
    state.open(path.map(PathBuf::from))
}

/// 当前工作区信息；还没打开时返回 `null`。
#[tauri::command]
pub fn workspace_status(
    state: tauri::State<'_, AppState>,
) -> Result<Option<WorkspaceStatus>, IpcError> {
    state.status()
}

/// 应用版本与默认工作区路径。
#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        default_workspace: workspace_root().display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::testing::TempWorkspace;

    #[test]
    fn commands_need_an_open_workspace_first() {
        let state = AppState::new();
        let error = state.with(|_| Ok(())).expect_err("还没打开工作区");
        assert_eq!(error.kind, "workspaceNotOpen");
        assert!(state.status().expect("查状态").is_none());
    }

    #[test]
    fn opening_a_workspace_reports_the_root_and_counts_purged_trash() {
        let temp = TempWorkspace::new("state");
        let state = AppState::new();
        let status = state
            .open(Some(temp.path().to_path_buf()))
            .expect("打开工作区");

        assert_eq!(status.root, temp.path().display().to_string());
        assert_eq!(status.purged_on_open, 0);
        assert!(state.status().expect("查状态").is_some());
        assert_eq!(
            state
                .with(|workspace| Ok(workspace.root().to_path_buf()))
                .expect("取工作区"),
            temp.path()
        );
    }

    #[test]
    fn app_info_points_at_the_default_workspace() {
        let info = app_info();
        assert!(!info.version.is_empty());
        assert!(!info.default_workspace.is_empty());
    }
}
