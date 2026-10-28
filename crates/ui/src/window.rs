#![forbid(unsafe_code)]

//! 界面要窗口做的、egui 自己做不到的那几件事。
//!
//! 界面层只认这个词表：切最大化、现在是不是最大化。拖动与拉边不需要平台帮忙，
//! 用 egui 的位置与尺寸命令逐帧做，各平台通用。具体实现放在平台翻译层
//! （notecevia-os），这里不出现任何平台细节。平台没有实现就返回 None，
//! 界面层退回 egui 自己的通用命令，所以缺了这一层窗口仍然能用。

/// 平台补上的窗口行为。
pub trait WindowHooks {
    /// 在屏幕坐标处弹出系统窗口菜单。标题栏里没有控件的地方右键就该出这个。
    fn show_system_menu(&self, x: f32, y: f32) -> bool;

    /// 切最大化或还原，返回切完之后是不是最大化。
    fn toggle_maximize(&self) -> Option<bool>;

    /// 现在是不是最大化。
    fn is_maximized(&self) -> Option<bool>;
}

/// 没有平台实现时用它：什么都不管。
impl WindowHooks for () {
    fn show_system_menu(&self, _x: f32, _y: f32) -> bool {
        false
    }

    fn toggle_maximize(&self) -> Option<bool> {
        None
    }

    fn is_maximized(&self) -> Option<bool> {
        None
    }
}
