//! 自绘窗口外观的激活。窗口先以原生装饰隐藏启动，这里换成自绘并露出窗口；
//! 换不过去就把原生装饰恢复回来再露出，用户至少能看到窗口。

use std::fmt::Display;

use tauri::WebviewWindow;
use tauri_plugin_decoration::WebviewWindowExt;

async fn restore_and_show(
    window: &WebviewWindow,
    activation_error: impl Display,
) -> Result<&'static str, String> {
    let restore_error = window.restore_decoration().await.err();
    let show_error = window.show().err();

    match (restore_error, show_error) {
        (None, None) => Ok("native"),
        (Some(restore), None) => Err(format!(
            "自绘窗口失败（{activation_error}）；恢复原生装饰也失败（{restore}），但窗口已经露出"
        )),
        (None, Some(show)) => Err(format!(
            "自绘窗口失败（{activation_error}）；原生兜底也露不出来（{show}）"
        )),
        (Some(restore), Some(show)) => Err(format!(
            "自绘窗口失败（{activation_error}）；恢复原生装饰失败（{restore}）；露出兜底窗口也失败（{show}）"
        )),
    }
}

/// 换成自绘窗口并露出窗口。返回 `custom` 或 `native`，前端据此决定标题栏。
#[tauri::command]
pub async fn activate_and_show(window: WebviewWindow) -> Result<&'static str, String> {
    if let Err(error) = window.activate_decoration().await {
        return restore_and_show(&window, error).await;
    }

    match window.show() {
        Ok(()) => Ok("custom"),
        Err(error) => restore_and_show(&window, error).await,
    }
}
