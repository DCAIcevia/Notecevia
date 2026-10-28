//! Win32：把可缩放边框加回去，以及问系统是不是最大化、让系统去切。
//!
//! 缩放走系统通道（DRAG_RESIZE），但"这个窗口可不可以拉边"是窗口样式决定的，
//! 无边框窗口得自己把 WS_THICKFRAME 补回去，系统的缩放请求才生效。

use core::ffi::c_void;

const GWL_STYLE: i32 = -16;
const WS_THICKFRAME: i32 = 0x0004_0000;

const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_FRAMECHANGED: u32 = 0x0020;

const TPM_RETURNCMD: u32 = 0x0100;
const TPM_NONOTIFY: u32 = 0x0080;
const WM_SYSCOMMAND: u32 = 0x0112;

const SW_MAXIMIZE: i32 = 3;
const SW_RESTORE: i32 = 9;

#[link(name = "user32")]
unsafe extern "system" {
    fn GetWindowLongW(window: *mut c_void, index: i32) -> i32;
    fn SetWindowLongW(window: *mut c_void, index: i32, value: i32) -> i32;
    fn SetWindowPos(
        window: *mut c_void,
        insert_after: *mut c_void,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn IsZoomed(window: *mut c_void) -> i32;
    fn GetSystemMenu(window: *mut c_void, revert: i32) -> *mut c_void;
    fn TrackPopupMenu(
        menu: *mut c_void,
        flags: u32,
        x: i32,
        y: i32,
        reserved: i32,
        window: *mut c_void,
        rect: *mut c_void,
    ) -> i32;
    fn PostMessageW(window: *mut c_void, message: u32, wparam: usize, lparam: isize) -> i32;
    fn ShowWindow(window: *mut c_void, command: i32) -> i32;
}

/// 句柄非零才当成窗口。句柄由 eframe 持有，在本进程存活期间一直有效。
fn window_of(hwnd: isize) -> Option<*mut c_void> {
    if hwnd == 0 {
        None
    } else {
        Some(hwnd as *mut c_void)
    }
}

pub fn allow_resize(hwnd: isize) -> bool {
    let Some(window) = window_of(hwnd) else {
        return false;
    };
    // SAFETY: 句柄来自 raw-window-handle，由 eframe 在整个运行期持有；
    // 这两个调用只读写窗口样式，不碰任何 Rust 管理的内存。
    unsafe {
        let style = GetWindowLongW(window, GWL_STYLE);
        if style == 0 {
            return false;
        }
        if style & WS_THICKFRAME != 0 {
            return true;
        }
        SetWindowLongW(window, GWL_STYLE, style | WS_THICKFRAME);
        // 样式改完要让系统重算边框，不然拉边缘仍然是拉客户端。
        SetWindowPos(
            window,
            core::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
    true
}

pub fn is_maximized(hwnd: isize) -> bool {
    let Some(window) = window_of(hwnd) else {
        return false;
    };
    // SAFETY: 同上，只读查询窗口状态。
    unsafe { IsZoomed(window) != 0 }
}

pub fn toggle_maximize(hwnd: isize) -> Option<bool> {
    let window = window_of(hwnd)?;
    let maximized = is_maximized(hwnd);
    // SAFETY: 同上。用系统的还原与最大化，不用 winit 自己记的那份状态。
    unsafe {
        ShowWindow(window, if maximized { SW_RESTORE } else { SW_MAXIMIZE });
    }
    Some(!maximized)
}
/// 在指针处弹出系统窗口菜单（还原、移动、大小、最小化、最大化、关闭）。
///
/// 微软的标题栏规范要求：标题栏里没有交互元素的地方右键就出这个菜单。
/// 菜单是系统的，所以由它绘制、由它处理选中项，我们只负责在正确的位置叫它出来。
pub fn show_system_menu(hwnd: isize, x: i32, y: i32) -> bool {
    let Some(window) = window_of(hwnd) else {
        return false;
    };
    // SAFETY: 句柄来自 raw-window-handle，由 eframe 持有；菜单句柄由系统管理，
    // 这里只是在指针位置把它显示出来，选中项的后果交给系统。
    unsafe {
        let menu = GetSystemMenu(window, 0);
        if menu.is_null() {
            return false;
        }
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY,
            x,
            y,
            0,
            window,
            core::ptr::null_mut(),
        );
        if command != 0 {
            PostMessageW(window, WM_SYSCOMMAND, command as usize, 0);
        }
    }
    true
}
