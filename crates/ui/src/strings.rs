#![forbid(unsafe_code)]

//! 界面文案。视图里不写死字符串，改文案只改这里。

/// 顶栏右侧的保存状态。
pub const UNSAVED: &str = "未保存";
pub const SAVED: &str = "已保存";

/// 画布底部的操作提示。
pub const BOARD_HINT: &str = "双击空白开始写 · 点别处确认 · 滚轮平移 · Ctrl+滚轮缩放 · Delete 删块 · F2 千块压测 · F3 视图复位";
