//! 导入导出命令：容器与自包含 HTML。
//!
//! 导出写到工作区的 `exports/` 目录，返回绝对路径字符串；显式给的文件名只是文件名，
//! 目录由这里拼，所以带路径的输入一律拒绝。容器导入回来是一张新画板，编号由 `store`
//! 分配，原来的编号不覆盖。

use std::fs;
use std::path::PathBuf;

use notecevia_store::Workspace;

use super::error::IpcError;
use super::state::AppState;

/// 导出文件的去处：工作区根下的一层目录，不存在就建。
const EXPORTS_DIR: &str = "exports";

/// 标题里不能当文件名的字符，换成下划线才拿来用。
const BAD_NAME_CHARS: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// Windows 上的保留设备名；拿来当文件名会写向设备或直接失败。
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// 把一张画板导出成容器文件，返回文件的绝对路径。
#[tauri::command]
pub fn export_container(
    state: tauri::State<'_, AppState>,
    board_id: u64,
    file_name: Option<String>,
) -> Result<String, IpcError> {
    state.with(|workspace| export_container_to(workspace, board_id, file_name.as_deref()))
}

/// 把一张画板导出成自包含 HTML，返回文件的绝对路径。
#[tauri::command]
pub fn export_html(
    state: tauri::State<'_, AppState>,
    board_id: u64,
    file_name: Option<String>,
) -> Result<String, IpcError> {
    state.with(|workspace| export_html_to(workspace, board_id, file_name.as_deref()))
}

/// 把一个容器文件导进来，返回新画板的编号。
#[tauri::command]
pub fn import_container(state: tauri::State<'_, AppState>, path: String) -> Result<u64, IpcError> {
    state.with(|workspace| Ok(workspace.import_container(&path)?))
}

/// 导出容器，返回路径。
fn export_container_to(
    workspace: &Workspace,
    board_id: u64,
    file_name: Option<&str>,
) -> Result<String, IpcError> {
    let path = export_path(workspace, board_id, file_name, "container")?;
    workspace.export_container(board_id, &path)?;
    Ok(path.display().to_string())
}

/// 导出 HTML，返回路径。
fn export_html_to(
    workspace: &Workspace,
    board_id: u64,
    file_name: Option<&str>,
) -> Result<String, IpcError> {
    let path = export_path(workspace, board_id, file_name, "html")?;
    workspace.export_html(board_id, &path)?;
    Ok(path.display().to_string())
}

/// 导出文件的绝对路径。显式名按原样用，不给它补扩展名。
fn export_path(
    workspace: &Workspace,
    board_id: u64,
    file_name: Option<&str>,
    extension: &str,
) -> Result<PathBuf, IpcError> {
    let name = match file_name {
        Some(name) => file_name_only(name)?.to_owned(),
        None => {
            let title = workspace.load(board_id)?.board.title;
            default_file_name(board_id, &title, extension)
        }
    };
    let dir = workspace.root().join(EXPORTS_DIR);
    fs::create_dir_all(&dir).map_err(|error| IpcError::new("io", error.to_string()))?;
    Ok(dir.join(name))
}

/// 默认文件名：`<编号>-<标题>.<扩展名>`。
fn default_file_name(board_id: u64, title: &str, extension: &str) -> String {
    format!("{board_id}-{}.{extension}", clean_file_stem(title))
}

/// 标题里当不了文件名的字符换成 `_`，再去掉首尾空白与结尾的点。
fn clean_file_stem(title: &str) -> String {
    let replaced: String = title
        .chars()
        .map(|character| {
            if BAD_NAME_CHARS.contains(&character) || character.is_control() {
                '_'
            } else {
                character
            }
        })
        .collect();
    let cleaned = replaced
        .trim()
        .trim_end_matches(['.', ' '])
        .trim()
        .to_owned();
    if cleaned.is_empty() {
        "画板".to_owned()
    } else {
        cleaned
    }
}

/// 显式文件名只接受纯文件名：空、`.`、`..`、带分隔符、Windows 保留名、以点或空格
/// 结尾的都拒绝。写出去的文件必须落在一个普通文件名上。
fn file_name_only(name: &str) -> Result<&str, IpcError> {
    if name.is_empty() {
        return Err(IpcError::new("badFileName", "文件名不能为空"));
    }
    if name == "." || name == ".." || name.contains(['/', '\\']) {
        return Err(IpcError::new("badFileName", "文件名不能带路径"));
    }
    if name.chars().any(char::is_control) {
        return Err(IpcError::new("badFileName", "文件名不能含控制字符"));
    }
    if name.ends_with(['.', ' ']) {
        return Err(IpcError::new("badFileName", "文件名不能以点或空格结尾"));
    }
    let stem = name.split('.').next().unwrap_or(name);
    if RESERVED_NAMES
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(stem))
    {
        return Err(IpcError::new("badFileName", "文件名是系统保留名"));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use notecevia_core::Document;

    use super::super::testing::TempWorkspace;
    use super::*;

    /// 存一张有标题的空画板，返回它的编号。
    fn saved_board(workspace: &Workspace, title: &str) -> u64 {
        let board_id = workspace.next_board().expect("下一个画板编号");
        workspace
            .save_at(board_id, &Document::new(title), 1_000)
            .expect("存画板");
        board_id
    }

    #[test]
    fn container_export_can_be_imported_as_a_new_board() {
        let temp = TempWorkspace::new("transfer-container");
        let board_id = saved_board(&temp.workspace, "带走这张画板");

        let exported =
            PathBuf::from(export_container_to(&temp.workspace, board_id, None).expect("导出容器"));
        let exports = temp.workspace.root().join(EXPORTS_DIR);
        assert!(exported.is_absolute(), "返回的应该是绝对路径");
        assert!(exported.exists(), "导出后文件应该存在");
        assert_eq!(
            exported.parent(),
            Some(exports.as_path()),
            "导出文件应该落在 exports/ 下"
        );

        let imported = temp
            .workspace
            .import_container(&exported)
            .expect("导入容器");
        assert_ne!(imported, board_id, "导入应该是新编号，不覆盖原画板");
        let document = temp.workspace.load(imported).expect("读导入的画板");
        assert_eq!(document.board.title, "带走这张画板");
    }

    #[test]
    fn html_export_starts_with_a_document_marker() {
        let temp = TempWorkspace::new("transfer-html");
        let board_id = saved_board(&temp.workspace, "导出成网页");
        let exported =
            PathBuf::from(export_html_to(&temp.workspace, board_id, None).expect("导出 HTML"));

        let bytes = fs::read(&exported).expect("读导出文件");
        let head = String::from_utf8_lossy(&bytes)
            .trim_start()
            .to_ascii_lowercase();
        assert!(
            head.starts_with("<!doctype") || head.starts_with("<html"),
            "HTML 开头不对：{}",
            head.chars().take(32).collect::<String>()
        );
    }

    #[test]
    fn explicit_file_name_with_a_path_is_rejected() {
        let temp = TempWorkspace::new("transfer-bad-name");
        let board_id = saved_board(&temp.workspace, "不许跑出去");

        let error = export_container_to(&temp.workspace, board_id, Some("../x"))
            .expect_err("带路径的文件名应该被拒绝");

        assert_eq!(error.kind, "badFileName");
        assert!(
            !temp.workspace.root().join("x").exists(),
            "不能在工作区根下写出文件"
        );
        assert!(
            !temp
                .workspace
                .root()
                .join(EXPORTS_DIR)
                .join("..")
                .join("x")
                .exists(),
            "不能写到 exports 以外"
        );
    }

    #[test]
    fn reserved_and_dot_ending_names_are_rejected() {
        let temp = TempWorkspace::new("transfer-reserved");
        let board_id = saved_board(&temp.workspace, "标题");

        for name in ["CON", "con.txt", "LPT1", "尾部点.", "尾部空格 "] {
            let error = export_container_to(&temp.workspace, board_id, Some(name))
                .expect_err("这个文件名应该被拒绝");
            assert_eq!(error.kind, "badFileName", "{name} 应该报 badFileName");
        }

        // 标题清洗后为空时给占位名，别拼出 "1-.container"。
        let empty_title = saved_board(&temp.workspace, "  ");
        let exported = export_container_to(&temp.workspace, empty_title, None).expect("导出");
        let file_name = Path::new(&exported)
            .file_name()
            .and_then(|name| name.to_str())
            .expect("文件名");
        assert!(
            file_name.ends_with("-画板.container"),
            "占位名不对：{file_name}"
        );
    }

    #[test]
    fn explicit_file_name_is_used_without_adding_an_extension() {
        let temp = TempWorkspace::new("transfer-explicit-name");
        let board_id = saved_board(&temp.workspace, "自定义文件名");

        let exported =
            export_container_to(&temp.workspace, board_id, Some("带去别处.backup")).expect("导出");

        let name = Path::new(&exported)
            .file_name()
            .and_then(|name| name.to_str())
            .expect("文件名");
        assert_eq!(name, "带去别处.backup", "显式名应该原样使用");
        assert!(Path::new(&exported).exists());
    }

    #[test]
    fn default_file_name_keeps_the_board_title() {
        let temp = TempWorkspace::new("transfer-title-name");
        let board_id = saved_board(&temp.workspace, "中文 标题/带斜杠");

        let exported = export_container_to(&temp.workspace, board_id, None).expect("导出容器");

        let name = Path::new(&exported)
            .file_name()
            .and_then(|name| name.to_str())
            .expect("文件名");
        assert!(name.starts_with("1-"), "默认名应该以编号开头：{name}");
        assert!(name.ends_with(".container"), "默认名应该带扩展名：{name}");
        assert!(
            name.contains("中文 标题_带斜杠"),
            "标题里的字应该保留、斜杠换成下划线：{name}"
        );
    }
}
