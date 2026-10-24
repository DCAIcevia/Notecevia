#![forbid(unsafe_code)]

//! 界面主题：颜色与尺寸只在这里出现。

use egui::Color32;

/// 一套画布配色。浅色与深色都要成立，界面代码只取这里的值。
pub struct Palette {
    pub canvas: Color32,
    pub block_fill: Color32,
    pub media_fill: Color32,
    pub edge: Color32,
    pub hover_edge: Color32,
    pub text: Color32,
    pub muted: Color32,
}

/// 世界坐标里的尺寸。绘制时乘以缩放，画面上才不会随窗口变大变小。
pub const BLOCK_FONT: f32 = 15.0;
pub const SMALL_FONT: f32 = 12.0;
/// 块的最小宽高，保证拉不没。
pub const MIN_BLOCK_WIDTH: f32 = 48.0;
pub const MIN_BLOCK_HEIGHT: f32 = 24.0;
/// 选中后贴着边能拉伸的宽度（屏幕像素）。
pub const GRAB_EDGE: f32 = 5.0;
/// 角外面这段区间用来旋转。
pub const GRAB_BAND: f32 = 18.0;
pub const BLOCK_RADIUS: f32 = 8.0;
pub const EDGE_WIDTH: f32 = 1.0;
pub const HINT_INSET: f32 = 12.0;
pub const HINT_PAD: f32 = 6.0;

/// 界面里能选的文字字号，单位是世界坐标里的点。
pub const TEXT_SIZES: [f32; 6] = [12.0, 14.0, 16.0, 20.0, 26.0, 34.0];

/// 调色板里先摆出来的常见颜色。想用别的就在同一个面板里自己调。
pub const TEXT_COLORS: [[u8; 3]; 7] = [
    [0, 0, 0],
    [110, 110, 118],
    [198, 60, 54],
    [206, 132, 40],
    [58, 140, 88],
    [44, 108, 200],
    [124, 92, 214],
];

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            canvas: Color32::from_rgb(24, 24, 27),
            block_fill: Color32::from_rgb(33, 33, 37),
            media_fill: Color32::from_rgb(43, 43, 48),
            edge: Color32::from_rgb(52, 52, 58),
            hover_edge: Color32::from_rgb(78, 78, 86),
            text: Color32::from_rgb(233, 233, 236),
            muted: Color32::from_rgb(142, 142, 150),
        }
    } else {
        Palette {
            canvas: Color32::from_rgb(242, 242, 244),
            block_fill: Color32::from_rgb(255, 255, 255),
            media_fill: Color32::from_rgb(235, 235, 238),
            edge: Color32::from_rgb(224, 224, 229),
            hover_edge: Color32::from_rgb(196, 196, 203),
            text: Color32::from_rgb(28, 28, 31),
            muted: Color32::from_rgb(110, 110, 118),
        }
    }
}
