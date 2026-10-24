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
    pub selected_edge: Color32,
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

/// 界面里能选的文字颜色。None 表示跟随主题，浅色深色都好看。
pub const TEXT_COLORS: [Option<[u8; 3]>; 6] = [
    None,
    Some([206, 84, 76]),
    Some([206, 146, 62]),
    Some([74, 154, 100]),
    Some([70, 120, 214]),
    Some([142, 116, 214]),
];

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            canvas: Color32::from_rgb(18, 19, 21),
            block_fill: Color32::from_rgb(32, 34, 38),
            media_fill: Color32::from_rgb(25, 27, 31),
            edge: Color32::from_rgb(48, 51, 57),
            hover_edge: Color32::from_rgb(78, 83, 92),
            selected_edge: Color32::from_rgb(104, 152, 232),
            text: Color32::from_rgb(228, 230, 234),
            muted: Color32::from_rgb(132, 138, 148),
        }
    } else {
        Palette {
            canvas: Color32::from_rgb(246, 246, 244),
            block_fill: Color32::from_rgb(255, 255, 255),
            media_fill: Color32::from_rgb(238, 238, 235),
            edge: Color32::from_rgb(219, 219, 214),
            hover_edge: Color32::from_rgb(186, 186, 180),
            selected_edge: Color32::from_rgb(40, 96, 200),
            text: Color32::from_rgb(28, 30, 34),
            muted: Color32::from_rgb(110, 114, 122),
        }
    }
}
