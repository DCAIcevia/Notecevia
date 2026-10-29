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
mod caption;
#[cfg(windows)]
mod windows;

/// 标题栏上的一个矩形，物理像素，原点在客户区左上角。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// 左边界。
    pub x: f32,
    /// 上边界。
    pub y: f32,
    /// 宽度。
    pub w: f32,
    /// 高度。
    pub h: f32,
}

/// 标题栏里交给系统接手的按钮矩形，物理像素，原点在客户区左上角。
///
/// 没有的按钮写 None：那片区域不命中系统行为，界面照常自己画、自己处理。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CaptionZones {
    /// 窗口图标。
    pub icon: Option<Rect>,
    /// 最大化按钮。
    pub maximize: Option<Rect>,
}

/// 系统接手的最大化按钮上指针的状态，供界面画按钮的悬停与按下。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CaptionPointer {
    /// 指针在最大化按钮上。
    pub hovered: bool,
    /// 左键在最大化按钮上按着。
    pub pressed: bool,
}

#[cfg(any(windows, test))]
mod caption_hit {
    use super::CaptionZones;

    /// 客户端物理坐标点落在哪片区域。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Hit {
        /// 不在交给系统的区域里，鼠标消息照常进客户区。
        Client,
        /// 图标区域：系统按标题栏图标处理。
        SysMenu,
        /// 最大化按钮区域：系统按最大化按钮处理。
        Maximize,
    }

    /// 纯命中测试：只看区域与点，不碰窗口，也不碰系统。
    ///
    /// 点落在矩形内指左、上边界算在内，右、下边界算在外。
    /// 图标区域与最大化区域重叠时图标优先；两者由界面给出，不应重叠。
    pub(crate) fn hit_test(zones: CaptionZones, x: f32, y: f32) -> Hit {
        if zones.icon.is_some_and(|rect| inside(rect, x, y)) {
            Hit::SysMenu
        } else if zones.maximize.is_some_and(|rect| inside(rect, x, y)) {
            Hit::Maximize
        } else {
            Hit::Client
        }
    }

    fn inside(rect: super::Rect, x: f32, y: f32) -> bool {
        x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h
    }

    #[cfg(test)]
    mod tests {
        use super::{Hit, hit_test};
        use crate::CaptionZones;

        fn rect(x: f32, y: f32, w: f32, h: f32) -> crate::Rect {
            crate::Rect { x, y, w, h }
        }

        fn zones() -> CaptionZones {
            CaptionZones {
                icon: Some(rect(8.0, 8.0, 32.0, 32.0)),
                maximize: Some(rect(100.0, 8.0, 46.0, 32.0)),
            }
        }

        #[test]
        fn icon_interior_is_sysmenu() {
            assert_eq!(hit_test(zones(), 24.0, 24.0), Hit::SysMenu);
        }

        #[test]
        fn maximize_interior_is_maximize() {
            assert_eq!(hit_test(zones(), 123.0, 24.0), Hit::Maximize);
        }

        #[test]
        fn elsewhere_is_client() {
            assert_eq!(hit_test(zones(), 70.0, 24.0), Hit::Client);
            assert_eq!(hit_test(zones(), 500.0, 500.0), Hit::Client);
        }

        #[test]
        fn missing_zones_are_all_client() {
            let empty = CaptionZones {
                icon: None,
                maximize: None,
            };
            assert_eq!(hit_test(empty, 24.0, 24.0), Hit::Client);
            assert_eq!(hit_test(empty, 123.0, 24.0), Hit::Client);
            let only_icon = CaptionZones {
                icon: Some(rect(8.0, 8.0, 32.0, 32.0)),
                maximize: None,
            };
            assert_eq!(hit_test(only_icon, 123.0, 24.0), Hit::Client);
        }

        #[test]
        fn left_and_top_edges_belong_to_the_zone() {
            assert_eq!(hit_test(zones(), 8.0, 8.0), Hit::SysMenu);
            assert_eq!(hit_test(zones(), 100.0, 8.0), Hit::Maximize);
        }

        #[test]
        fn right_and_bottom_edges_fall_outside() {
            assert_eq!(hit_test(zones(), 40.0, 24.0), Hit::Client);
            assert_eq!(hit_test(zones(), 24.0, 40.0), Hit::Client);
            assert_eq!(hit_test(zones(), 146.0, 24.0), Hit::Client);
            assert_eq!(hit_test(zones(), 123.0, 40.0), Hit::Client);
        }

        #[test]
        fn icon_wins_when_regions_overlap() {
            let overlapping = CaptionZones {
                icon: Some(rect(8.0, 8.0, 32.0, 32.0)),
                maximize: Some(rect(8.0, 8.0, 32.0, 32.0)),
            };
            assert_eq!(hit_test(overlapping, 24.0, 24.0), Hit::SysMenu);
        }
    }
}

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
    ///
    /// 同时补上系统菜单、最小化框、最大化框：贴靠布局与系统菜单项都以这些样式为前提。
    pub fn allow_resize(&self) -> bool {
        windows::allow_resize(self.hwnd)
    }

    /// 请 DWM 给窗口画圆角。
    pub fn round_window_corners(&self) -> bool {
        windows::round_window_corners(self.hwnd)
    }

    /// 指针的屏幕坐标，逻辑点。
    pub fn cursor_position(&self) -> Option<(f32, f32)> {
        let point = windows::cursor_position()?;
        Some((point.x as f32, point.y as f32))
    }

    /// 物理上鼠标左键还按着吗。
    pub fn primary_button_down(&self) -> bool {
        windows::primary_button_down()
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

    /// 界面每帧调用：告诉系统图标与最大化按钮在哪。
    ///
    /// 第一次调用时挂上窗口过程钩子，之后只更新矩形。
    pub fn set_caption_zones(&self, zones: CaptionZones) {
        caption::set_zones(self.hwnd, zones);
    }

    /// 系统接手区域内指针的悬停与按下；钩子没装成返回 None。
    pub fn caption_pointer(&self) -> Option<CaptionPointer> {
        caption::pointer(self.hwnd)
    }

    /// 标题栏的命中测试是否真的由这一层接管。
    pub fn handles_caption(&self) -> bool {
        caption::handles(self.hwnd)
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
    pub fn primary_button_down(&self) -> bool {
        false
    }

    /// 这个平台没有实现窗口行为。
    pub fn round_window_corners(&self) -> bool {
        false
    }

    /// 这个平台没有实现窗口行为。
    pub fn cursor_position(&self) -> Option<(f32, f32)> {
        None
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

    /// 这个平台没有实现标题栏命中测试。
    pub fn set_caption_zones(&self, _zones: CaptionZones) {}

    /// 这个平台没有实现标题栏命中测试。
    pub fn caption_pointer(&self) -> Option<CaptionPointer> {
        None
    }

    /// 这个平台没有实现标题栏命中测试。
    pub fn handles_caption(&self) -> bool {
        false
    }
}
