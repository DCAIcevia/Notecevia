#![forbid(unsafe_code)]

//! 界面图标：用画笔画的线条图标，不依赖图标字体。
//!
//! 每个图标的坐标都在 0..1 的方框里，画到给定矩形上，所以同一套图标在任何尺寸下
//! 都不会变形。线宽、颜色由调用方给，图标本身不决定。

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

/// 界面上用到图标。
#[derive(Clone, Copy)]
pub enum Icon {
    PanelLeft,
    PanelRight,
    Plus,
    More,
    Search,
    Export,
    Doc,
    Image,
    Page,
    Volume,
    Shelf,
    Shape,
    Highlight,
    Text,
    Table,
    Audio,
    Play,
}

/// 把图标画在 `rect` 里。`rect` 应当是个正方形。
pub fn draw(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let size = rect.width().min(rect.height());
    let box_rect = Rect::from_center_size(rect.center(), Vec2::splat(size));
    let stroke = Stroke::new((size / 16.0).max(1.0), color);
    let p = |x: f32, y: f32| -> Pos2 {
        box_rect.min + Vec2::new(x * box_rect.width(), y * box_rect.height())
    };
    let line = |a: Pos2, b: Pos2| painter.line_segment([a, b], stroke);
    let dot = |x: f32, y: f32, r: f32| painter.circle_filled(p(x, y), r * size, color);

    match icon {
        Icon::PanelLeft => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.15, 0.2), p(0.85, 0.8)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Inside,
            );
            line(p(0.4, 0.2), p(0.4, 0.8));
        }
        Icon::PanelRight => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.15, 0.2), p(0.85, 0.8)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Inside,
            );
            line(p(0.6, 0.2), p(0.6, 0.8));
        }
        Icon::Plus => {
            // 两臂撑到 0.18..0.82：十字只有两条线，占格和别的图标一样宽才不显小。
            line(p(0.5, 0.18), p(0.5, 0.82));
            line(p(0.18, 0.5), p(0.82, 0.5));
        }
        Icon::More => {
            dot(0.26, 0.5, 0.055);
            dot(0.5, 0.5, 0.055);
            dot(0.74, 0.5, 0.055);
        }
        Icon::Search => {
            painter.circle_stroke(p(0.45, 0.45), 0.22 * size, stroke);
            line(p(0.62, 0.62), p(0.82, 0.82));
        }
        Icon::Export => {
            line(p(0.5, 0.74), p(0.5, 0.24));
            painter.add(egui::Shape::line(
                vec![p(0.33, 0.4), p(0.5, 0.22), p(0.67, 0.4)],
                stroke,
            ));
            painter.add(egui::Shape::line(
                vec![p(0.22, 0.66), p(0.22, 0.8), p(0.78, 0.8), p(0.78, 0.66)],
                stroke,
            ));
        }
        Icon::Doc => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.26, 0.16), p(0.74, 0.84)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Inside,
            );
            line(p(0.38, 0.4), p(0.62, 0.4));
            line(p(0.38, 0.54), p(0.62, 0.54));
            line(p(0.38, 0.68), p(0.52, 0.68));
        }
        Icon::Page => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.27, 0.12), p(0.73, 0.88)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Inside,
            );
            line(p(0.37, 0.32), p(0.63, 0.32));
            line(p(0.37, 0.46), p(0.63, 0.46));
            line(p(0.37, 0.6), p(0.52, 0.6));
        }
        Icon::Volume => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.18, 0.12), p(0.45, 0.88)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Inside,
            );
            painter.rect_stroke(
                Rect::from_min_max(p(0.49, 0.16), p(0.82, 0.88)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Inside,
            );
            line(p(0.24, 0.12), p(0.24, 0.88));
        }
        Icon::Shelf => {
            line(p(0.16, 0.82), p(0.84, 0.82));
            line(p(0.26, 0.3), p(0.26, 0.82));
            line(p(0.4, 0.16), p(0.4, 0.82));
            line(p(0.54, 0.36), p(0.54, 0.82));
            line(p(0.68, 0.22), p(0.68, 0.82));
        }
        Icon::Text => {
            line(p(0.22, 0.26), p(0.78, 0.26));
            line(p(0.5, 0.26), p(0.5, 0.78));
        }
        Icon::Table => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.16, 0.2), p(0.84, 0.8)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Inside,
            );
            line(p(0.16, 0.5), p(0.84, 0.5));
            line(p(0.5, 0.2), p(0.5, 0.8));
        }
        Icon::Shape => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.14, 0.2), p(0.56, 0.6)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Inside,
            );
            painter.circle_stroke(p(0.66, 0.66), 0.17 * size, stroke);
        }
        Icon::Highlight => {
            painter.add(egui::Shape::convex_polygon(
                vec![p(0.28, 0.66), p(0.62, 0.3), p(0.74, 0.42), p(0.4, 0.78)],
                Color32::TRANSPARENT,
                stroke,
            ));
            line(p(0.24, 0.86), p(0.46, 0.78));
        }
        Icon::Image => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.16, 0.22), p(0.84, 0.78)),
                CornerRadius::same(1),
                stroke,
                StrokeKind::Inside,
            );
            painter.circle_stroke(p(0.37, 0.4), 0.06 * size, stroke);
            painter.add(egui::Shape::line(
                vec![
                    p(0.2, 0.72),
                    p(0.42, 0.52),
                    p(0.6, 0.66),
                    p(0.72, 0.55),
                    p(0.8, 0.62),
                ],
                stroke,
            ));
        }
        Icon::Audio => {
            line(p(0.3, 0.4), p(0.3, 0.6));
            line(p(0.42, 0.28), p(0.42, 0.72));
            line(p(0.54, 0.36), p(0.54, 0.64));
            line(p(0.66, 0.24), p(0.66, 0.76));
        }
        Icon::Play => {
            painter.add(egui::Shape::convex_polygon(
                vec![p(0.4, 0.28), p(0.72, 0.5), p(0.4, 0.72)],
                color,
                Stroke::NONE,
            ));
        }
    }
}
