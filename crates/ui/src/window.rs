#![forbid(unsafe_code)]

//! 界面要窗口做的、egui 自己做不到的那几件事。
//!
//! 界面层只认这个词表：切最大化、现在是不是最大化。拖动与拉边不需要平台帮忙，
//! 用 egui 的位置与尺寸命令逐帧做，各平台通用。具体实现放在平台翻译层
//! （notecevia-os），这里不出现任何平台细节。平台没有实现就返回 None，
//! 界面层退回 egui 自己的通用命令，所以缺了这一层窗口仍然能用。

/// 标题栏里准备交给系统处理的两块矩形，物理像素，客户区左上角为原点。
///
/// 界面自己画窗口按钮，但有两块必须让系统接管才有系统行为：图标（单击出系统
/// 菜单、双击关窗）与最大化按钮（Win11 贴靠布局浮窗只认系统认得的按钮区域）。
/// 界面每帧把位置报上去，系统按它的规矩处理；其余按钮仍由界面自己响应。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CaptionZones {
    /// 窗口图标所在的那一格。
    pub icon: Option<egui::Rect>,
    /// 最大化（或还原）按钮所在的那一格。
    pub maximize: Option<egui::Rect>,
}

/// 系统接管的那一块此刻的指针状态。
///
/// 交给系统之后，界面收不到这些位置的指针事件，悬停与按下只能问系统。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CaptionPointer {
    /// 指针停在这一块上。
    pub hovered: bool,
    /// 指针按在这一块上。
    pub pressed: bool,
}

/// 平台补上的窗口行为。
pub trait WindowHooks {
    /// 在屏幕坐标处弹出系统窗口菜单。标题栏里没有控件的地方右键就该出这个。
    fn show_system_menu(&self, x: f32, y: f32) -> bool;

    /// 物理上鼠标左键还按着吗。窗口被搬动时抬起事件可能丢失，得问系统。
    fn primary_button_down(&self) -> Option<bool>;

    /// 指针的屏幕坐标，按界面点数算。
    ///
    /// 必须问系统要，不能用"窗口局部坐标加窗口位置"去算：窗口自己在动的时候，
    /// 局部坐标会跟着变，用它算位移就成了回路，尺寸会一路滚上去。
    fn cursor_position(&self) -> Option<(f32, f32)>;

    /// 切最大化或还原，返回切完之后是不是最大化。
    fn toggle_maximize(&self) -> Option<bool>;

    /// 现在是不是最大化。
    fn is_maximized(&self) -> Option<bool>;

    /// 把标题栏里要交给系统的两块报上去。每帧调用，坐标是物理像素。
    fn caption_zones(&self, _zones: CaptionZones) {}

    /// 系统是不是已经接管了图标与最大化按钮。接管了，界面就别抢这两个点击。
    fn handles_caption(&self) -> bool {
        false
    }

    /// 系统接管的那一块此刻悬停、按下没有。
    fn caption_pointer(&self) -> Option<CaptionPointer> {
        None
    }
}

/// 没有平台实现时用它：什么都不管。
impl WindowHooks for () {
    fn show_system_menu(&self, _x: f32, _y: f32) -> bool {
        false
    }

    fn cursor_position(&self) -> Option<(f32, f32)> {
        None
    }

    fn primary_button_down(&self) -> Option<bool> {
        None
    }

    fn toggle_maximize(&self) -> Option<bool> {
        None
    }

    fn is_maximized(&self) -> Option<bool> {
        None
    }
}
