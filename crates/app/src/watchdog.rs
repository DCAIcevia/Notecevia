//! 前端心跳：界面脚本停止响应时，窗口不能变成关不掉的白板。
//!
//! 前端每几秒打一次心跳；超过阈值没有心跳、并且主窗口是可见的，就写一份
//! 崩溃记录并退出进程。只在窗口可见时计时，避免最小化后浏览器节流定时器
//! 把正常程序当成挂死。

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use notecevia_store::workspace_root;

/// 前端上一次心跳的时间。
pub struct Heartbeat {
    last: Mutex<Instant>,
}

impl Default for Heartbeat {
    fn default() -> Self {
        Self::new()
    }
}

impl Heartbeat {
    pub fn new() -> Self {
        Self {
            last: Mutex::new(Instant::now()),
        }
    }

    pub fn beat(&self) {
        if let Ok(mut last) = self.last.lock() {
            *last = Instant::now();
        }
    }

    /// 距离上一次心跳过了多久；锁坏了按刚刚打过处理，不误杀。
    pub fn idle_for(&self) -> Duration {
        self.last.lock().map_or(Duration::ZERO, |last| {
            Instant::now().saturating_duration_since(*last)
        })
    }
}

/// 判定：距离上一次心跳超过阈值就算挂死。纯函数，便于测。
pub fn is_stale(idle: Duration, limit: Duration) -> bool {
    idle > limit
}

/// 写一份崩溃记录，然后退出进程。
fn report_and_exit(app: &AppHandle, reason: &str) {
    let directory = workspace_root();
    let _ = std::fs::create_dir_all(&directory);
    let path = directory.join("crash.log");
    let _ = std::fs::write(&path, format!("{reason}\n"));
    eprintln!("{reason}");
    app.exit(1);
    // exit 已经请求了事件循环退出；这里兜底，避免命令线程卡住进程。
    std::process::exit(1);
}

/// 起一个后台线程盯着心跳。窗口不可见时不判定。
pub fn spawn(app: AppHandle, limit: Duration) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(3));
            let visible = app
                .get_webview_window("main")
                .map(|window| window.is_visible().unwrap_or(true))
                .unwrap_or(true);
            if !visible {
                continue;
            }
            let Some(heartbeat) = app.try_state::<Heartbeat>() else {
                return;
            };
            if is_stale(heartbeat.idle_for(), limit) {
                report_and_exit(
                    &app,
                    &format!(
                        "界面脚本 {} 秒没有心跳，窗口已关闭（详见 crash.log）",
                        limit.as_secs()
                    ),
                );
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_beat_inside_the_limit_is_not_stale() {
        assert!(!is_stale(Duration::from_secs(10), Duration::from_secs(60)));
    }

    #[test]
    fn silence_past_the_limit_is_stale() {
        assert!(is_stale(Duration::from_secs(61), Duration::from_secs(60)));
    }

    #[test]
    fn a_fresh_heartbeat_reports_near_zero_idle() {
        let heartbeat = Heartbeat::new();
        heartbeat.beat();
        assert!(heartbeat.idle_for() < Duration::from_secs(1));
    }
}
