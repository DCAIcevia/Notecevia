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

/// 块的种类名，菜单、工具条与无障碍文本共用。
pub const KIND_TEXT: &str = "文字";
pub const KIND_IMAGE: &str = "图片";
pub const KIND_VIDEO: &str = "视频";
pub const KIND_AUDIO: &str = "音频";
pub const KIND_TABLE: &str = "表格";
pub const KIND_SHAPE: &str = "形状";
pub const KIND_HIGHLIGHT: &str = "高亮";

/// 画布底部的操作提示。
pub const BOARD_HINT: &str = "双击空白开始写 · 拖入图片/视频/音频 · Shift 点多选 · Ctrl+Z 撤销 · Ctrl+S 保存 · 滚轮平移 · Ctrl+滚轮缩放";

/// 空画板的引导：一句下一步，加一行快捷键。
pub const EMPTY_BOARD: &str = "双击空白开始写";
pub const EMPTY_BOARD_HINT: &str = "拖入图片、视频、音频";

/// 选中块后工具条上的动作。删除不在这里：只走键盘 Delete。
pub const DUPLICATE: &str = "复制";
pub const LOCK: &str = "锁定";
pub const UNLOCK: &str = "解锁";
pub const HIDE: &str = "隐藏";
pub const LAYER_FRONT: &str = "置顶";
pub const LAYER_BACK: &str = "置底";
pub const ALIGN_LEFT: &str = "左对齐";
pub const ALIGN_CENTER_X: &str = "水平居中";
pub const ALIGN_RIGHT: &str = "右对齐";
pub const ALIGN_TOP: &str = "顶对齐";
pub const ALIGN_CENTER_Y: &str = "垂直居中";
pub const ALIGN_BOTTOM: &str = "底对齐";
pub const DISTRIBUTE_H: &str = "横向等距";
pub const DISTRIBUTE_V: &str = "纵向等距";

/// 形状种类。
pub const SHAPE_RECT: &str = "矩形";
pub const SHAPE_ELLIPSE: &str = "椭圆";
pub const SHAPE_LINE: &str = "直线";

/// 表格。
pub const TABLE_ADD_ROW: &str = "加行";
pub const TABLE_ADD_COL: &str = "加列";

/// 新建块菜单。
pub const NEW_TEXT: &str = "文字";
pub const NEW_IMAGE: &str = "图片";
pub const NEW_TABLE: &str = "表格";
pub const NEW_SHAPE: &str = "形状";
pub const NEW_HIGHLIGHT: &str = "高亮";
pub const INSERT_MEDIA_HINT: &str = "把文件拖进画布即可插入";

/// 画布上的状态提示。
pub const UNDONE: &str = "已撤销";
pub const REDONE: &str = "已重做";
pub const COPIED: &str = "已复制";
pub const BLOCK_DELETED: &str = "已删除";
pub const MEDIA_MISSING: &str = "媒体文件找不到了";
pub const MEDIA_OPEN_HINT: &str = "双击用系统播放器打开";

/// 顶栏的返回入口。
pub const BACK_TO_HOME: &str = "全部画板";

/// 保存与打开失败的文案。错误信息 = 发生了什么 + 怎么办。
pub fn save_failed(error: impl std::fmt::Display) -> String {
    format!("保存失败：{error}")
}

pub fn open_failed(error: impl std::fmt::Display) -> String {
    format!("打开失败，本次不写盘：{error}")
}

pub fn workspace_failed(path: impl std::fmt::Display, error: impl std::fmt::Display) -> String {
    format!("工作区打不开（{path}）：{error}")
}

/// 首页。
pub const HOME_TITLE: &str = "全部画板";
pub const HOME_NEW: &str = "新建画板";
pub const HOME_OPEN: &str = "打开";
pub const HOME_RENAME: &str = "重命名";
pub const HOME_DELETE: &str = "删除";
pub const HOME_EXPORT: &str = "导出";
pub const HOME_EXPORT_CONTAINER: &str = "导出容器";
pub const HOME_EXPORT_HTML: &str = "导出 HTML";
pub const HOME_SEARCH_HINT: &str = "搜索标题、标签与正文";
pub const HOME_EMPTY: &str = "还没有笔记";
pub const HOME_EMPTY_HINT: &str = "点右上角 ＋ 新建";
pub const HOME_NO_RESULTS: &str = "没有匹配的笔记";
pub const HOME_NO_RESULTS_HINT: &str = "换个词试试，或者清空搜索";
pub const HOME_TAGS: &str = "标签";
pub const HOME_ADD_TAG: &str = "加标签";
pub const HOME_TAG_FILTER: &str = "按标签筛选";
pub const HOME_CONFIRM_DELETE: &str = "删掉这条笔记？内容会移进回收站，可以手动找回";
pub const HOME_CANCEL: &str = "取消";
pub const HOME_CONFIRM: &str = "确认删除";
pub const HOME_UNTITLED: &str = "未命名笔记";
pub const HOME_BLOCK_COUNT: &str = "块";
pub const HOME_UPDATED_JUST_NOW: &str = "刚更新";
pub const HOME_UPDATED_TODAY: &str = "今天更新";
pub const HOME_UPDATED_YESTERDAY: &str = "昨天更新";
pub const HOME_UPDATED_DAYS: &str = "天前更新";

/// 首页操作之后的状态。
pub const BOARD_CREATED: &str = "已新建笔记";
pub const BOARD_DELETED: &str = "笔记已移进回收站";
pub const BOARD_RENAMED: &str = "已重命名";
pub const TAGS_SAVED: &str = "标签已保存";
pub const EXPORTED: &str = "已导出";
pub const IMPORTED: &str = "已导入笔记";
pub const RECOVERED: &str = "上次没写完的临时文件已清理";

pub fn export_failed(error: impl std::fmt::Display) -> String {
    format!("导出失败：{error}")
}
pub fn exported(path: impl std::fmt::Display) -> String {
    format!("已导出：{path}")
}

pub const CANNOT_SAVE: &str = "不能保存：工作区不可用";
pub const UNSUPPORTED_FILE: &str = "这个文件类型放不进画布";
pub const MEDIA_READ_FAILED: &str = "读不了这个文件";
/// 新建块菜单的标签。
pub const NEW_MENU: &str = "新建";

/// 隐藏块的反向操作。
pub const SHOW: &str = "显示";

/// 图片填充方式。
pub const FIT_COVER: &str = "填充";
pub const FIT_CONTAIN: &str = "适应";
pub const FIT_FILL: &str = "拉伸";

/// 形状与高亮的属性标签。
pub const SHAPE_FILL: &str = "填充色";
pub const SHAPE_STROKE: &str = "描边色";
pub const HIGHLIGHT_OPACITY: &str = "透明度";
/// 首页的无障碍与下一步文案。
pub const HOME_CLEAR_SEARCH: &str = "清空搜索";
pub const HOME_MORE: &str = "更多操作";
pub const HOME_TAG_REMOVE: &str = "删除标签";
/// 导出当前画面。
pub const EXPORT_IMAGE: &str = "导出图片";
pub const EXPORTING: &str = "正在导出…";
pub fn image_exported(path: impl std::fmt::Display) -> String {
    format!("已导出图片：{path}")
}

/// 首页标签筛选。
pub const HOME_FILTER_CLEAR: &str = "清除筛选";
pub const HOME_FILTER_ALL: &str = "全部";
/// 首页刷新列表。
pub const HOME_REFRESH: &str = "刷新";
pub const REFRESHED: &str = "列表已刷新";

/// 外壳：应用名。
pub const BRAND: &str = "Notecevia";
/// 外壳：搜索框占位。
pub const SHELL_SEARCH: &str = "搜索";
/// 外壳：收起左侧边栏。
pub const SHELL_TOGGLE_LIST: &str = "收起左侧边栏";
/// 外壳：收起资源栏。
pub const SHELL_TOGGLE_ASSETS: &str = "收起资源栏";
/// 外壳：没有打开的笔记。
pub const SHELL_NO_NOTE: &str = "还没有打开的笔记";
/// 外壳：新建笔记。
pub const SHELL_NEW_NOTE: &str = "新建笔记";
/// 资源栏：没有资源时的拖入提示。
pub const ASSETS_DROP: &str = "拖入文件";

/// 画布栏：新建块按钮的无障碍标签。
pub const NEW_BLOCK: &str = "新建块";
/// 画布栏上的缩放百分比。数字等宽，缩放时不左右跳。
pub fn zoom_percent(zoom: f32) -> String {
    format!("{:.0}%", zoom * 100.0)
}

/// 左侧边栏：新建按钮的无障碍标签。
pub const SIDEBAR_NEW: &str = "新建笔记";
/// 左侧边栏行里的相对时间。界面拿不到本地时区，所以只写相对时刻。
pub const SIDEBAR_TIME_JUST_NOW: &str = "刚刚";
pub const SIDEBAR_TIME_MINUTES: &str = "分钟前";
pub const SIDEBAR_TIME_HOURS: &str = "小时前";
pub const SIDEBAR_TIME_YESTERDAY: &str = "昨天";
pub const SIDEBAR_TIME_DAYS: &str = "天前";
/// 左侧边栏：标签输入框的提示与标签之间的分隔符。
pub const SIDEBAR_TAGS_HINT: &str = "标签，逗号分开";
pub const TAG_SEPARATOR: char = '，';

/// 新建笔记的默认标题。
pub const UNTITLED_NOTE: &str = "无标题笔记";

/// 已经开着一张空笔记时再点新建。
pub const BOARD_ALREADY_EMPTY: &str = "已经是空笔记了";

/// 菜单栏：菜单名。
pub const MENU_FILE: &str = "文件";
pub const MENU_EDIT: &str = "编辑";
pub const MENU_SELECT: &str = "选择";
pub const MENU_VIEW: &str = "视图";
pub const MENU_HELP: &str = "帮助";

/// 菜单栏：菜单项。
pub const MENU_NEW_NOTE: &str = "新建笔记";
pub const MENU_EXPORT_HTML: &str = "导出 HTML";
pub const MENU_EXPORT_CONTAINER: &str = "导出容器";
pub const MENU_CLOSE_NOTE: &str = "关闭笔记";
pub const MENU_QUIT: &str = "退出";
pub const MENU_UNDO: &str = "撤销";
pub const MENU_REDO: &str = "重做";
pub const MENU_DELETE: &str = "删除选中的块";
pub const MENU_SELECT_ALL: &str = "全选";
pub const MENU_CLEAR_SELECT: &str = "取消选择";
pub const MENU_HIDE_LIST: &str = "收起左侧边栏";
pub const MENU_HIDE_ASSETS: &str = "收起资源栏";
pub const MENU_ABOUT: &str = "关于 Notecevia";

/// 关于：只在状态行显示一次。
pub fn about() -> String {
    format!("Notecevia {}", crate::version())
}
