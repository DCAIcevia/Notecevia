//! 画布之外的界面：顶栏与选中块的操作条。
//!
//! 这里的东西守同一条规矩：没有内容的时候不占地方。顶栏只放画板名与保存状态；
//! 格式操作只在选中一块时浮在它上方，取消选中就消失。

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use notecevia_core::board::TextBlock;

use crate::{strings, theme};

/// 顶栏高度。画布从这条线以下开始。
pub const TOP_BAR_HEIGHT: f32 = 40.0;

const TOOLBAR_HEIGHT: f32 = 32.0;
const TOOLBAR_GAP: f32 = 8.0;
const BUTTON: f32 = 24.0;
const SWATCH: f32 = 12.0;
const SWATCH_GAP: f32 = 8.0;
const PAD: f32 = 8.0;
const RADIUS: u8 = 6;

/// 顶栏：画板名在左，保存状态在右，底下一条细线与画布分界。
pub fn top_bar(ui: &egui::Ui, palette: &theme::Palette, rect: Rect, title: &str, state: &str) {
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, palette.canvas);
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, palette.edge),
    );

    let font = FontId::proportional(theme::SMALL_FONT);
    painter.text(
        rect.left_center() + Vec2::new(theme::HINT_INSET, 0.0),
        Align2::LEFT_CENTER,
        title,
        font.clone(),
        palette.text,
    );
    if !state.is_empty() {
        painter.text(
            rect.right_center() - Vec2::new(theme::HINT_INSET, 0.0),
            Align2::RIGHT_CENTER,
            state,
            font,
            palette.muted,
        );
    }
}

/// 选中块上方的格式操作条。返回这次有没有改动样式。
pub fn style_toolbar(
    ui: &egui::Ui,
    palette: &theme::Palette,
    block: Rect,
    style: &mut TextBlock,
) -> bool {
    let width = PAD * 2.0
        + BUTTON * 2.0
        + SWATCH_GAP
        + theme::TEXT_COLORS.len() as f32 * (SWATCH + SWATCH_GAP)
        - SWATCH_GAP;
    let area = ui.clip_rect();
    // 默认浮在块上方；上方放不下就挪到块下方，不躲到屏幕外。
    let above = block.top() - TOOLBAR_GAP - TOOLBAR_HEIGHT;
    let top = if above >= area.top() {
        above
    } else {
        block.bottom() + TOOLBAR_GAP
    };
    let left = (block.center().x - width / 2.0).clamp(
        area.left() + 4.0,
        (area.right() - width - 4.0).max(area.left() + 4.0),
    );
    let bar = Rect::from_min_size(Pos2::new(left, top), Vec2::new(width, TOOLBAR_HEIGHT));

    let painter = ui.painter();
    painter.rect(
        bar,
        CornerRadius::same(RADIUS),
        palette.block_fill,
        Stroke::new(1.0, palette.edge),
        StrokeKind::Inside,
    );

    let mut changed = false;
    let mut x = bar.left() + PAD;
    let middle = bar.center().y;

    for (label, step) in [("A−", -1), ("A+", 1)] {
        let button =
            Rect::from_center_size(Pos2::new(x + BUTTON / 2.0, middle), Vec2::splat(BUTTON));
        let response = ui.interact(button, ui.scope_id().with(("size", label)), Sense::click());
        if response.hovered() {
            painter.rect_filled(button, CornerRadius::same(RADIUS), palette.media_fill);
        }
        painter.text(
            button.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(theme::SMALL_FONT),
            palette.text,
        );
        if response.clicked() {
            style.size = stepped_size(style.size, step);
            changed = true;
        }
        x += BUTTON;
    }

    x += SWATCH_GAP;
    for (index, color) in theme::TEXT_COLORS.iter().enumerate() {
        let center = Pos2::new(x + SWATCH / 2.0, middle);
        let dot = Rect::from_center_size(center, Vec2::splat(SWATCH));
        let response = ui.interact(
            dot.expand(SWATCH_GAP / 2.0),
            ui.scope_id().with(("color", index)),
            Sense::click(),
        );
        let fill = match color {
            Some([r, g, b]) => Color32::from_rgb(*r, *g, *b),
            None => palette.text,
        };
        painter.circle_filled(center, SWATCH / 2.0, fill);
        if style.color == *color {
            painter.circle_stroke(center, SWATCH / 2.0 + 2.0, Stroke::new(1.0, palette.text));
        } else if response.hovered() {
            painter.circle_stroke(
                center,
                SWATCH / 2.0 + 2.0,
                Stroke::new(1.0, palette.hover_edge),
            );
        }
        if response.clicked() {
            style.color = *color;
            changed = true;
        }
        x += SWATCH + SWATCH_GAP;
    }

    let _ = strings::UNSAVED;
    changed
}

/// 在字号档位里上下走一格。没设过字号时从默认档开始。
fn stepped_size(current: Option<f32>, step: i32) -> Option<f32> {
    let sizes = theme::TEXT_SIZES;
    let current = current.unwrap_or(theme::BLOCK_FONT);
    let index = sizes
        .iter()
        .position(|size| (size - current).abs() < 0.5)
        .unwrap_or(2);
    let next = (index as i32 + step).clamp(0, sizes.len() as i32 - 1) as usize;
    Some(sizes[next])
}
