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
pub const BLOCK_PAD: f32 = 12.0;
pub const BLOCK_RADIUS: f32 = 8.0;
pub const EDGE_WIDTH: f32 = 1.0;
pub const SELECTED_EDGE_WIDTH: f32 = 2.0;
pub const HINT_INSET: f32 = 12.0;

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
