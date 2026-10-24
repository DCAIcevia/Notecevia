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
const COLOR_BUTTON: f32 = 26.0;
const GAP: f32 = 6.0;
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
    let width = PAD * 2.0 + BUTTON * 2.0 + GAP * 2.0 + COLOR_BUTTON * 2.0;
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

    // 文字色。点开是调色板：自动 + 常见色 + 自己调。
    x += GAP;
    let button = Rect::from_center_size(
        Pos2::new(x + COLOR_BUTTON / 2.0, middle),
        Vec2::splat(COLOR_BUTTON),
    );
    let response = ui.interact(button, ui.scope_id().with("text-colour"), Sense::click());
    paint_text_colour_button(painter, button, palette, style.color, response.hovered());
    changed |= colour_popup(palette, &response, &mut style.color);
    x += COLOR_BUTTON;

    // 底色。跟文字色同一个调色板。
    x += GAP;
    let button = Rect::from_center_size(
        Pos2::new(x + COLOR_BUTTON / 2.0, middle),
        Vec2::splat(COLOR_BUTTON),
    );
    let response = ui.interact(
        button,
        ui.scope_id().with("background-colour"),
        Sense::click(),
    );
    paint_background_colour_button(
        painter,
        button,
        palette,
        style.background,
        response.hovered(),
    );
    changed |= colour_popup(palette, &response, &mut style.background);

    changed
}

/// 调色板：自动（跟随主题）、一排常见色，再加一个能自己调的取色器。
fn colour_popup(
    palette: &theme::Palette,
    button: &egui::Response,
    target: &mut Option<[u8; 3]>,
) -> bool {
    let mut changed = false;
    egui::Popup::from_toggle_button_response(button)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(target.is_none(), strings::COLOUR_AUTO)
                    .clicked()
                {
                    *target = None;
                    changed = true;
                    ui.close();
                }
                for colour in theme::TEXT_COLORS {
                    let swatch = Rect::from_min_size(ui.cursor().min, Vec2::splat(BUTTON / 1.2));
                    let response = ui.allocate_rect(swatch, Sense::click());
                    ui.painter().rect_filled(
                        swatch.shrink(2.0),
                        CornerRadius::same(2),
                        Color32::from_rgb(colour[0], colour[1], colour[2]),
                    );
                    if response.clicked() {
                        *target = Some(colour);
                        changed = true;
                        ui.close();
                    }
                }
            });

            let mut current = target.map_or(palette.text, |[r, g, b]| Color32::from_rgb(r, g, b));
            if egui::color_picker::color_picker_color32(
                ui,
                &mut current,
                egui::color_picker::Alpha::Opaque,
            ) {
                let [r, g, b, _] = current.to_srgba_unmultiplied();
                *target = Some([r, g, b]);
                changed = true;
            }
        });
    changed
}

fn paint_text_colour_button(
    painter: &egui::Painter,
    button: Rect,
    palette: &theme::Palette,
    colour: Option<[u8; 3]>,
    hovered: bool,
) {
    if hovered {
        painter.rect_filled(button, CornerRadius::same(RADIUS), palette.media_fill);
    }
    let centre = button.center();
    let radius = 6.0;
    match colour {
        Some([r, g, b]) => {
            painter.circle_filled(centre, radius, Color32::from_rgb(r, g, b));
        }
        // 没设过就是跟随主题，画一个空心圈。
        None => {
            painter.circle_stroke(centre, radius, Stroke::new(1.0, palette.text));
        }
    }
}

fn paint_background_colour_button(
    painter: &egui::Painter,
    button: Rect,
    palette: &theme::Palette,
    colour: Option<[u8; 3]>,
    hovered: bool,
) {
    if hovered {
        painter.rect_filled(button, CornerRadius::same(RADIUS), palette.media_fill);
    }
    let square = Rect::from_center_size(button.center(), Vec2::splat(12.0));
    match colour {
        Some([r, g, b]) => {
            painter.rect_filled(square, CornerRadius::same(2), Color32::from_rgb(r, g, b));
        }
        // 没底色就是一个空框加一道斜线，表示没有。
        None => {
            painter.rect_stroke(
                square,
                CornerRadius::same(2),
                Stroke::new(1.0, palette.text),
                StrokeKind::Inside,
            );
            painter.line_segment(
                [square.left_top(), square.right_bottom()],
                Stroke::new(1.0, palette.text),
            );
        }
    }
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
