#![forbid(unsafe_code)]

//! 界面文案。视图里不写死字符串，改文案只改这里。

/// 顶栏右侧的保存状态。
pub const UNSAVED: &str = "未保存";
pub const SAVED: &str = "已保存";

/// 字体列表里的「默认」与过滤框提示。
pub const FONT_DEFAULT: &str = "默认字体";
pub const FONT_FILTER: &str = "筛选字体";

/// 调色板里的「跟随主题」。
pub const COLOUR_AUTO: &str = "自动";

/// 画布底部的操作提示。
pub const BOARD_HINT: &str = "双击空白开始写 · 点别处确认 · Ctrl+N 新建 · Ctrl+S 保存 · 滚轮平移 · Ctrl+滚轮缩放 · Delete 删块 · F2 千块压测 · F3 视图复位";
