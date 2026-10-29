//! 标题栏命中测试：把图标与最大化按钮交给系统。
//!
//! 无边框窗口的标题栏画在客户区里，系统默认看不见这些按钮。唯一的接入点是窗口过程里的
//! WM_NCHITTEST：图标矩形回 HTSYSMENU、最大化矩形回 HTMAXBUTTON，其余交回默认处理。
//! 系统拿到的贴靠布局浮窗、系统菜单与双击关窗都从这两个命中码上长出来。
//!
//! 指针落在系统接管的区域后，客户区收不到鼠标消息，所以最大化按钮的悬停与按下由
//! WM_NC* 系列消息记下来，再让界面取走画按钮。图标没有悬停反馈，只回报最大化按钮。
//!
//! 状态放在 thread_local + Cell 里：窗口过程只在创建窗口的线程上被调用，eframe 的界面
//! 循环也在该线程上，set_zones、pointer 与窗口过程不会跨线程，因此不需要锁。

use core::cell::Cell;
use core::ffi::c_void;

use crate::caption_hit::{Hit, hit_test};
use crate::windows::{Point, cursor_position, primary_button_down};
use crate::{CaptionPointer, CaptionZones};

const WM_NCDESTROY: u32 = 0x0082;
const WM_KILLFOCUS: u32 = 0x0008;
const WM_NCHITTEST: u32 = 0x0084;
const WM_NCMOUSEMOVE: u32 = 0x00A0;
const WM_NCLBUTTONDOWN: u32 = 0x00A1;
const WM_NCLBUTTONUP: u32 = 0x00A2;
const WM_NCMOUSELEAVE: u32 = 0x02A2;

const HTCLIENT: isize = 1;
const HTSYSMENU: isize = 3;
const HTMAXBUTTON: isize = 9;

// WM_NCMOUSELEAVE 不会自己来：第一次 WM_NCMOUSEMOVE 时登记，收到离开后重新登记。
const TME_LEAVE: u32 = 0x0000_0002;
const TME_NONCLIENT: u32 = 0x0000_0010;

/// 子类化登记号。一个窗口只挂一个本模块的钩子。
const SUBCLASS_ID: usize = 0x4E43_5654;

type SubclassProc =
    unsafe extern "system" fn(*mut c_void, u32, usize, isize, usize, usize) -> isize;

#[link(name = "comctl32")]
unsafe extern "system" {
    fn SetWindowSubclass(
        window: *mut c_void,
        proc: Option<SubclassProc>,
        id: usize,
        data: usize,
    ) -> i32;
    fn RemoveWindowSubclass(window: *mut c_void, proc: Option<SubclassProc>, id: usize) -> i32;
    fn DefSubclassProc(window: *mut c_void, message: u32, wparam: usize, lparam: isize) -> isize;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn ScreenToClient(window: *mut c_void, point: *mut Point) -> i32;
    fn TrackMouseEvent(event: *mut TrackMouseEventData) -> i32;
}

#[repr(C)]
struct TrackMouseEventData {
    size: u32,
    flags: u32,
    window: *mut c_void,
    hover_time: u32,
}

#[derive(Clone, Copy)]
struct State {
    window: isize,
    zones: CaptionZones,
    installed: bool,
    tracking: bool,
    hovered: bool,
    pressed: bool,
}

const EMPTY: State = State {
    window: 0,
    zones: CaptionZones {
        icon: None,
        maximize: None,
    },
    installed: false,
    tracking: false,
    hovered: false,
    pressed: false,
};

thread_local! {
    static CAPTION: Cell<State> = const { Cell::new(EMPTY) };
}

/// 记下按钮矩形；第一次调用时装上窗口过程钩子。
pub fn set_zones(hwnd: isize, zones: CaptionZones) {
    if hwnd == 0 {
        return;
    }
    CAPTION.with(|cell| {
        let state = cell.get();
        if state.installed && state.window == hwnd {
            let mut next = state;
            next.zones = zones;
            cell.set(next);
            return;
        }
        let window = hwnd as *mut c_void;
        // SAFETY: 句柄来自 raw-window-handle，窗口存活期间一直有效；回调是静态函数，
        // 不捕获栈上数据，登记后由系统持有到 WM_NCDESTROY 里移除。
        let installed =
            unsafe { SetWindowSubclass(window, Some(caption_proc), SUBCLASS_ID, 0) != 0 };
        if installed {
            cell.set(State {
                window: hwnd,
                zones,
                installed: true,
                tracking: false,
                hovered: false,
                pressed: false,
            });
        }
    });
}

/// 系统接手区域里的指针状态；这个窗口没装上钩子就返回 None。
pub fn pointer(hwnd: isize) -> Option<CaptionPointer> {
    if hwnd == 0 {
        return None;
    }
    CAPTION.with(|cell| {
        let state = cell.get();
        if !state.installed || state.window != hwnd {
            return None;
        }
        Some(CaptionPointer {
            hovered: state.hovered,
            // 按下消息可能被系统菜单截走，抬起消息不一定回得来；
            // 同时问一次物理按键，菜单关掉之后按下状态不会挂着。
            pressed: state.pressed && primary_button_down(),
        })
    })
}

/// 这个窗口的标题栏命中测试是否已由本模块接管。
pub fn handles(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    CAPTION.with(|cell| {
        let state = cell.get();
        state.installed && state.window == hwnd
    })
}

unsafe extern "system" fn caption_proc(
    window: *mut c_void,
    message: u32,
    wparam: usize,
    lparam: isize,
    _id: usize,
    _data: usize,
) -> isize {
    match message {
        WM_NCHITTEST => {
            // SAFETY: 句柄由系统交给窗口过程，消息处理期间有效；消息参数不含指针。
            let default = unsafe { DefSubclassProc(window, message, wparam, lparam) };
            // 先看系统自己的结论：缩放边框等已有命中不要抢。
            if default != HTCLIENT {
                return default;
            }
            // lParam 低字是横坐标、高字是纵坐标，各有符号位；多显示器会出现负坐标。
            let (x, y) = (low_word(lparam), high_word(lparam));
            match zone_at(window, x, y) {
                Hit::SysMenu => HTSYSMENU,
                Hit::Maximize => HTMAXBUTTON,
                Hit::Client => default,
            }
        }
        WM_NCMOUSEMOVE => {
            track_leave(window);
            update_hover(window);
            // SAFETY: 同 WM_NCHITTEST。
            unsafe { DefSubclassProc(window, message, wparam, lparam) }
        }
        WM_NCLBUTTONDOWN => {
            update_pressed(window, true, wparam);
            // SAFETY: 同上。按下交给系统处理，最大化、菜单都由它执行。
            unsafe { DefSubclassProc(window, message, wparam, lparam) }
        }
        WM_NCLBUTTONUP => {
            update_pressed(window, false, wparam);
            // SAFETY: 同上。
            unsafe { DefSubclassProc(window, message, wparam, lparam) }
        }
        WM_NCMOUSELEAVE => {
            CAPTION.with(|cell| {
                let mut state = cell.get();
                if state.window != window as isize {
                    return;
                }
                state.tracking = false;
                state.hovered = false;
                state.pressed = false;
                cell.set(state);
            });
            // SAFETY: 同上。
            unsafe { DefSubclassProc(window, message, wparam, lparam) }
        }
        WM_KILLFOCUS => {
            clear_pressed(window);
            // SAFETY: 同上。
            unsafe { DefSubclassProc(window, message, wparam, lparam) }
        }
        WM_NCDESTROY => {
            CAPTION.with(|cell| {
                if cell.get().window == window as isize {
                    cell.set(EMPTY);
                }
            });
            // SAFETY: 窗口还在销毁过程中，句柄有效；移除的是本文件登记的回调，
            // 移除之后再走默认处理，链条才完整。
            unsafe {
                RemoveWindowSubclass(window, Some(caption_proc), SUBCLASS_ID);
                DefSubclassProc(window, message, wparam, lparam)
            }
        }
        _ => {
            // SAFETY: 句柄由系统交给窗口过程，消息处理期间有效；消息参数原样交回系统。
            unsafe { DefSubclassProc(window, message, wparam, lparam) }
        }
    }
}

fn low_word(value: isize) -> i32 {
    ((value & 0xFFFF) as u16 as i16) as i32
}

fn high_word(value: isize) -> i32 {
    (((value >> 16) & 0xFFFF) as u16 as i16) as i32
}

/// 屏幕坐标换算成客户区物理坐标；换算失败按不在任何区域处理。
fn client_point(window: *mut c_void, x: i32, y: i32) -> Option<(i32, i32)> {
    let mut point = Point { x, y };
    // SAFETY: 句柄由系统交给窗口过程；point 是栈上的有效结构体，系统只读改写它。
    let ok = unsafe { ScreenToClient(window, &mut point) };
    (ok != 0).then_some((point.x, point.y))
}

fn zones_of(window: *mut c_void) -> CaptionZones {
    CAPTION.with(|cell| {
        let state = cell.get();
        if state.installed && state.window == window as isize {
            state.zones
        } else {
            CaptionZones {
                icon: None,
                maximize: None,
            }
        }
    })
}

fn zone_at(window: *mut c_void, screen_x: i32, screen_y: i32) -> Hit {
    let Some((x, y)) = client_point(window, screen_x, screen_y) else {
        return Hit::Client;
    };
    hit_test(zones_of(window), x as f32, y as f32)
}

fn update_hover(window: *mut c_void) {
    let Some(Point { x, y }) = cursor_position() else {
        return;
    };
    let Some((x, y)) = client_point(window, x, y) else {
        return;
    };
    let hovered = hit_test(zones_of(window), x as f32, y as f32) == Hit::Maximize;
    CAPTION.with(|cell| {
        let mut state = cell.get();
        if state.window != window as isize {
            return;
        }
        state.hovered = hovered;
        if !hovered {
            state.pressed = false;
        }
        cell.set(state);
    });
}

fn update_pressed(window: *mut c_void, pressed: bool, hit_code: usize) {
    let zones = zones_of(window);
    let on_button = hit_code == HTMAXBUTTON as usize && zones.maximize.is_some();
    CAPTION.with(|cell| {
        let mut state = cell.get();
        if state.window != window as isize {
            return;
        }
        state.pressed = pressed && on_button;
        if state.pressed {
            state.hovered = true;
        }
        cell.set(state);
    });
}

fn clear_pressed(window: *mut c_void) {
    CAPTION.with(|cell| {
        let mut state = cell.get();
        if state.window != window as isize {
            return;
        }
        state.pressed = false;
        cell.set(state);
    });
}

/// 第一次悬停时登记离开通知；WM_NCMOUSELEAVE 之后才重新登记。
fn track_leave(window: *mut c_void) {
    let first = CAPTION.with(|cell| {
        let mut state = cell.get();
        if state.window != window as isize || state.tracking {
            return false;
        }
        state.tracking = true;
        cell.set(state);
        true
    });
    if !first {
        return;
    }
    let mut event = TrackMouseEventData {
        size: core::mem::size_of::<TrackMouseEventData>() as u32,
        flags: TME_LEAVE | TME_NONCLIENT,
        window,
        hover_time: 0,
    };
    // SAFETY: event 是本栈上的有效结构体，size 按实际大小填写，系统只读它。
    let ok = unsafe { TrackMouseEvent(&mut event) };
    if ok == 0 {
        CAPTION.with(|cell| {
            let mut state = cell.get();
            if state.window == window as isize {
                state.tracking = false;
                cell.set(state);
            }
        });
    }
}
