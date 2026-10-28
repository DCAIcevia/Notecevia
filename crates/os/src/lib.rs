#![deny(unsafe_op_in_unsafe_fn)]
#![deny(rustdoc::broken_intra_doc_links)]

//! 系统窗口能力：界面把边框交给系统去掉了，这几件系统行为得补回来。
//!
//! 界面层只说"开始拖动""切成最大化"，具体怎么做只有这一层知道。
//! 没有实现的平台一律返回 false 或 None，界面层就退回 egui 自己的通用命令——
//! 所以它是加强项，不是依赖项：这一层不成立，窗口也还能用。
//!
//! 全库只有这里允许 unsafe：它是唯一直接和系统窗口打交道的边界。

#[cfg(windows)]
mod windows;

/// 由平台补上的那几件窗口行为。
#[cfg(windows)]
#[derive(Clone, Copy)]
pub struct Native {
    hwnd: isize,
}

/// 没有这些能力的平台：每个调用都说不支持。
#[cfg(not(windows))]
#[derive(Clone, Copy)]
pub struct Native;

#[cfg(windows)]
impl Native {
    /// 从窗口句柄建。不是 Win32 句柄就返回 None。
    pub fn attach(handle: raw_window_handle::RawWindowHandle) -> Option<Self> {
        match handle {
            raw_window_handle::RawWindowHandle::Win32(win32) => Some(Self {
                hwnd: win32.hwnd.get(),
            }),
            _ => None,
        }
    }

    /// 把系统认得的可缩放边框加回去：系统缩放要靠它才算数。
    pub fn allow_resize(&self) -> bool {
        windows::allow_resize(self.hwnd)
    }

    /// 在指针处弹出系统窗口菜单。
    pub fn show_system_menu(&self, x: i32, y: i32) -> bool {
        windows::show_system_menu(self.hwnd, x, y)
    }

    /// 切最大化或还原，返回切完之后是不是最大化。
    pub fn toggle_maximize(&self) -> Option<bool> {
        windows::toggle_maximize(self.hwnd)
    }

    /// 现在是不是最大化。问系统，不问缓存：缓存和真实窗口会不一致。
    pub fn is_maximized(&self) -> Option<bool> {
        Some(windows::is_maximized(self.hwnd))
    }
}

#[cfg(not(windows))]
impl Native {
    /// 这个平台没有实现窗口行为。
    pub fn attach(_handle: raw_window_handle::RawWindowHandle) -> Option<Self> {
        None
    }

    /// 这个平台没有实现窗口行为。
    pub fn allow_resize(&self) -> bool {
        false
    }

    /// 这个平台没有实现窗口行为。
    pub fn show_system_menu(&self, _x: i32, _y: i32) -> bool {
        false
    }

    /// 这个平台没有实现窗口行为。
    pub fn toggle_maximize(&self) -> Option<bool> {
        None
    }

    /// 这个平台没有实现窗口行为。
    pub fn is_maximized(&self) -> Option<bool> {
        None
    }
}
