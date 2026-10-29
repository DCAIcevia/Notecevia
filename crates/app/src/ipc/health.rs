//! 前端心跳。界面每几秒打一次，看门狗据此判断脚本还活着。

use crate::watchdog::Heartbeat;

/// 打一次心跳。失败没有副作用，前端也不必处理。
#[tauri::command]
pub fn heartbeat(heartbeat: tauri::State<'_, Heartbeat>) {
    heartbeat.beat();
}
