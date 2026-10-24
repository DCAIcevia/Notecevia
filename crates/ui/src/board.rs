#![forbid(unsafe_code)]

//! 画布：无限平面上的一堆块，加拖动、缩放与就地编辑。
//!
//! 绘制走屏幕坐标而不是整层变换：文本按缩放后的字号重新排版，放大后仍然清晰。
//! 代价是每帧要重排可见文字，所以绘制与交互都先按视口裁剪。

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use notecevia_core::board::{Document, Geometry};

use crate::{strings, theme};

/// 缩放范围。再小看不清，再大失去意义。
const MIN_ZOOM: f32 = 0.15;
const MAX_ZOOM: f32 = 6.0;
/// 新块的默认世界尺寸。
const NEW_TEXT_SIZE: Vec2 = Vec2::new(320.0, 120.0);
/// 视图复位后的状态。
const DEFAULT_ORIGIN: Pos2 = Pos2::new(160.0, 140.0);
const DEFAULT_ZOOM: f32 = 1.0;

/// 视口：世界坐标与屏幕坐标之间的换算。
struct View {
    /// 世界原点落在屏幕上的位置。
    origin: Pos2,
    zoom: f32,
}

impl View {
    fn restored() -> Self {
        Self {
            origin: DEFAULT_ORIGIN,
            zoom: DEFAULT_ZOOM,
        }
    }

    fn to_screen(&self, point: Pos2) -> Pos2 {
        self.origin + point.to_vec2() * self.zoom
    }

    fn to_world(&self, point: Pos2) -> Pos2 {
        ((point - self.origin) / self.zoom).to_pos2()
    }

    fn rect_to_screen(&self, rect: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(rect.min), self.to_screen(rect.max))
    }
}

/// 画布界面。数据在 Document 里，这里只管怎么显示、怎么改。
pub struct BoardApp {
    view: View,
    document: Document,
    editing: Option<u64>,
    selected: Option<u64>,
    hovered: Option<u64>,
    status: Option<String>,
    fps: f32,
    dirty: bool,
}

impl BoardApp {
    pub fn new(document: Document) -> Self {
        Self {
            view: View::restored(),
            document,
            editing: None,
            selected: None,
            hovered: None,
            status: None,
            fps: 0.0,
            dirty: false,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    /// 在右下角显示一条状态，比如保存结果。
    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = Some(status.into());
    }

    /// 存过盘之后调用，清掉未保存标记。
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let palette = theme::palette(ui.visuals().dark_mode);
        let rect = ui.max_rect();
        let clip = ui.clip_rect().intersect(rect);

        // 先登记背景，再登记块：后登记的在上，块会拿走落在它身上的点击。
        let background = ui.interact(rect, ui.scope_id().with("canvas"), Sense::click_and_drag());
        self.apply_input(ui, &background, rect);

        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, palette.canvas);
        self.interact_blocks(ui, clip);
        self.paint_blocks(ui, &palette, clip);
        self.edit_text(ui, &palette);
        self.paint_hud(ui, &palette, rect);
    }

    // ---- 输入 ----

    fn apply_input(&mut self, ui: &egui::Ui, background: &egui::Response, rect: Rect) {
        if background.dragged() {
            self.view.origin += background.drag_delta();
        }

        if background.clicked() {
            self.selected = None;
            self.editing = None;
        }

        if background.double_clicked()
            && let Some(pointer) = ui.input(|i| i.pointer.interact_pos())
        {
            let world = self.view.to_world(pointer);
            let block = self.document.add_text(Geometry::new(
                world.x,
                world.y,
                NEW_TEXT_SIZE.x,
                NEW_TEXT_SIZE.y,
            ));
            self.selected = Some(block);
            self.editing = Some(block);
            self.dirty = true;
        }

        if ui.rect_contains_pointer(rect) {
            let (scroll, zoom_delta, pointer) =
                ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos()));
            if let Some(pointer) = pointer {
                if zoom_delta != 1.0 {
                    self.zoom_at(pointer, zoom_delta);
                } else if scroll != Vec2::ZERO {
                    self.view.origin += scroll;
                }
            }
        }

        let (escape, delete, stress, reset_view) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Delete),
                i.key_pressed(egui::Key::F2),
                i.key_pressed(egui::Key::F3),
            )
        });

        if escape {
            self.editing = None;
        }
        if delete
            && !ui.ctx().egui_wants_keyboard_input()
            && let Some(block) = self.selected
        {
            self.document.remove_block(block);
            self.selected = None;
            self.dirty = true;
        }
        if stress {
            self.spawn_stress();
        }
        if reset_view {
            self.view = View::restored();
        }
    }

    /// 以指针为不动点缩放。
    fn zoom_at(&mut self, pointer: Pos2, factor: f32) {
        let world = self.view.to_world(pointer);
        self.view.zoom = (self.view.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.view.origin = pointer - world.to_vec2() * self.view.zoom;
    }

    // ---- 交互 ----

    fn interact_blocks(&mut self, ui: &egui::Ui, clip: Rect) {
        let mut responses = Vec::new();
        for index in 0..self.document.board.entries.len() {
            let entry = &self.document.board.entries[index];
            let screen = self.view.rect_to_screen(rect_of(entry.geometry));
            if !clip.intersects(screen) {
                continue;
            }
            let id = ui.scope_id().with(("block", entry.id));
            responses.push((index, ui.interact(screen, id, Sense::click_and_drag())));
        }

        self.hovered = None;
        for (index, response) in responses {
            let block = self.document.board.entries[index].id;
            if response.hovered() || response.dragged() {
                self.hovered = Some(block);
            }
            if response.dragged() && self.editing != Some(block) {
                let delta = response.drag_delta() / self.view.zoom;
                let entry = &mut self.document.board.entries[index];
                entry.geometry = translate(entry.geometry, delta);
                self.dirty = true;
            }
            if response.clicked() {
                self.selected = Some(block);
            }
            if response.double_clicked() {
                self.selected = Some(block);
                self.editing = Some(block);
            }
        }
    }

    // ---- 绘制 ----

    fn paint_blocks(&self, ui: &egui::Ui, palette: &theme::Palette, clip: Rect) {
        let painter = ui.painter().with_clip_rect(clip);
        let zoom = self.view.zoom;

        for entry in &self.document.board.entries {
            let screen = self.view.rect_to_screen(rect_of(entry.geometry));
            if !clip.intersects(screen) {
                continue;
            }

            let selected = self.selected == Some(entry.id);
            let hovered = self.hovered == Some(entry.id);
            let edge = if selected {
                palette.selected_edge
            } else if hovered {
                palette.hover_edge
            } else {
                palette.edge
            };
            let width = if selected {
                theme::SELECTED_EDGE_WIDTH
            } else {
                theme::EDGE_WIDTH
            } * zoom;
            let radius = CornerRadius::same((theme::BLOCK_RADIUS * zoom).round() as u8);

            painter.rect(
                screen,
                radius,
                palette.block_fill,
                Stroke::new(width, edge),
                StrokeKind::Inside,
            );

            if self.editing != Some(entry.id) {
                let text = self.document.text_of(entry.id);
                paint_text(&painter, screen, text, zoom, palette.text);
            }
        }
    }

    fn edit_text(&mut self, ui: &mut egui::Ui, palette: &theme::Palette) {
        let Some(block) = self.editing else {
            return;
        };
        let Some(index) = self
            .document
            .board
            .entries
            .iter()
            .position(|entry| entry.id == block)
        else {
            self.editing = None;
            return;
        };

        let zoom = self.view.zoom;
        let screen = self
            .view
            .rect_to_screen(rect_of(self.document.board.entries[index].geometry));
        let margin = theme::BLOCK_PAD * zoom;
        let font = FontId::proportional((theme::BLOCK_FONT * zoom).max(4.0));
        let desired_width = (screen.width() - 2.0 * margin).max(8.0);

        let response = {
            let text = &mut self.document.text_mut(block).text;
            let editor = egui::TextEdit::multiline(text)
                .id(ui.scope_id().with(("editor", block)))
                .font(font)
                .text_color(palette.text)
                .background_color(palette.block_fill)
                .margin(egui::Margin::same(margin.round() as i8))
                .frame(egui::Frame::NONE)
                .desired_width(desired_width)
                .desired_rows(1);
            ui.put(screen, editor)
        };

        if response.changed() {
            self.dirty = true;
        }
        if response.lost_focus() {
            self.editing = None;
        }
    }

    fn paint_hud(&mut self, ui: &egui::Ui, palette: &theme::Palette, rect: Rect) {
        let dt = ui.input(|i| i.stable_dt).max(1.0e-4);
        let instant = 1.0 / dt;
        self.fps = if self.fps == 0.0 {
            instant
        } else {
            self.fps * 0.9 + instant * 0.1
        };

        let painter = ui.painter();
        let font = FontId::proportional(theme::SMALL_FONT);
        let bottom = rect.left_bottom() + Vec2::new(theme::HINT_INSET, -theme::HINT_INSET);
        paint_chip(
            painter,
            bottom,
            Align2::LEFT_BOTTOM,
            strings::BOARD_HINT,
            &font,
            palette,
        );

        let mut right = format!("{:.0}%   {:.0} fps", self.view.zoom * 100.0, self.fps);
        if self.dirty {
            right.push_str("   未保存");
        }
        if let Some(status) = &self.status {
            right.push_str("   ");
            right.push_str(status);
        }
        let anchor = rect.right_bottom() + Vec2::new(-theme::HINT_INSET, -theme::HINT_INSET);
        paint_chip(
            painter,
            anchor,
            Align2::RIGHT_BOTTOM,
            &right,
            &font,
            palette,
        );
    }

    // ---- 内容 ----

    /// 一千块铺满当前视口与下方，用来量拖动与缩放时的帧率。
    pub fn spawn_stress(&mut self) {
        let columns = 24;
        let start = self.view.to_world(Pos2::ZERO) - Vec2::splat(20.0);
        for index in 0..1000 {
            let x = start.x + (index % columns) as f32 * 70.0;
            let y = start.y + (index / columns) as f32 * 55.0;
            let block = self.document.add_text(Geometry::new(x, y, 62.0, 47.0));
            self.document.text_mut(block).text = format!("块 {index}");
        }
        self.dirty = true;
    }
}

/// 画一条底部提示。叠在内容上时先铺一层画布底色，否则字和块糊在一起。
fn paint_chip(
    painter: &egui::Painter,
    anchor: Pos2,
    align: Align2,
    text: &str,
    font: &FontId,
    palette: &theme::Palette,
) {
    let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), palette.muted);
    let rect = align.anchor_size(anchor, galley.size());
    let [r, g, b, _] = palette.canvas.to_srgba_unmultiplied();
    let backdrop = Color32::from_rgba_unmultiplied(r, g, b, 224);
    let padding = Vec2::new(theme::HINT_PAD, theme::HINT_PAD / 2.0);
    painter.rect_filled(
        rect.expand2(padding),
        CornerRadius::same((theme::BLOCK_RADIUS * 0.5).round() as u8),
        backdrop,
    );
    painter.galley(rect.min, galley, palette.muted);
}

fn rect_of(geometry: Geometry) -> Rect {
    Rect::from_min_size(
        Pos2::new(geometry.x, geometry.y),
        Vec2::new(geometry.width, geometry.height),
    )
}

fn translate(geometry: Geometry, delta: Vec2) -> Geometry {
    Geometry::new(
        geometry.x + delta.x,
        geometry.y + delta.y,
        geometry.width,
        geometry.height,
    )
}

fn paint_text(painter: &egui::Painter, screen: Rect, text: &str, zoom: f32, color: Color32) {
    if text.is_empty() {
        return;
    }
    let font = FontId::proportional((theme::BLOCK_FONT * zoom).max(4.0));
    let pad = theme::BLOCK_PAD * zoom;
    let wrap = (screen.width() - 2.0 * pad).max(8.0);
    let galley = painter.layout(text.to_owned(), font, color, wrap);
    painter.galley(screen.min + Vec2::splat(pad), galley, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> BoardApp {
        BoardApp::new(Document::new("测试画板"))
    }

    #[test]
    fn world_and_screen_roundtrip() {
        let app = board();
        let world = Pos2::new(123.0, 45.0);
        let back = app.view.to_world(app.view.to_screen(world));
        assert!((back - world).length() < 0.001);
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let mut app = board();
        let pointer = Pos2::new(400.0, 300.0);
        let before = app.view.to_world(pointer);
        app.zoom_at(pointer, 1.8);
        let after = app.view.to_world(pointer);
        assert!((after - before).length() < 0.001);
        assert!((app.view.zoom - 1.8).abs() < 0.001);
    }

    #[test]
    fn zoom_stays_in_range() {
        let mut app = board();
        let pointer = Pos2::new(0.0, 0.0);
        for _ in 0..40 {
            app.zoom_at(pointer, 4.0);
        }
        assert!((app.view.zoom - MAX_ZOOM).abs() < 0.001);
        for _ in 0..40 {
            app.zoom_at(pointer, 0.25);
        }
        assert!((app.view.zoom - MIN_ZOOM).abs() < 0.001);
    }

    #[test]
    fn geometry_survives_a_translation() {
        let geometry = Geometry::new(10.0, 20.0, 30.0, 40.0);
        let moved = translate(geometry, Vec2::new(5.0, -5.0));
        assert_eq!(moved, Geometry::new(15.0, 15.0, 30.0, 40.0));
    }
}
