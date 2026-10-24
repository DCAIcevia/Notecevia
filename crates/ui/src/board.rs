#![forbid(unsafe_code)]

//! 画布：无限平面上的一堆块，加拖动、缩放与就地编辑。
//!
//! 绘制走屏幕坐标而不是整层变换：文本按缩放后的字号重新排版，放大后仍然清晰。
//! 代价是每帧要重排可见文字，所以绘制与交互都先按视口裁剪。

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::{strings, theme};

/// 缩放范围。再小看不清，再大失去意义。
const MIN_ZOOM: f32 = 0.15;
const MAX_ZOOM: f32 = 6.0;
/// 新块的默认世界尺寸。
const NEW_TEXT_SIZE: Vec2 = Vec2::new(320.0, 120.0);

/// 一块内容的种类。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Image,
    Media,
}

struct Block {
    id: u64,
    kind: Kind,
    /// 世界坐标里的矩形，与缩放无关。
    rect: Rect,
    text: String,
}

/// 视口：世界坐标与屏幕坐标之间的换算。
struct View {
    /// 世界原点落在屏幕上的位置。
    origin: Pos2,
    zoom: f32,
}

impl View {
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

/// 原型界面：一张画布加若干块内容。
pub struct BoardApp {
    view: View,
    blocks: Vec<Block>,
    editing: Option<u64>,
    selected: Option<u64>,
    hovered: Option<u64>,
    next_id: u64,
    texture: Option<egui::TextureHandle>,
    image_note: Option<String>,
    fps: f32,
}

impl BoardApp {
    /// 建立界面。image 给定时解码该文件，否则用程序生成的测试图。
    pub fn new(ctx: &egui::Context, image: Option<&std::path::Path>) -> Self {
        let mut app = Self {
            view: View {
                origin: Pos2::new(160.0, 140.0),
                zoom: 1.0,
            },
            blocks: Vec::new(),
            editing: None,
            selected: None,
            hovered: None,
            next_id: 1,
            texture: None,
            image_note: None,
            fps: 0.0,
        };
        app.load_image(ctx, image);
        app.reset_blocks();
        app
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
            let id = self.push_text(world, NEW_TEXT_SIZE, "");
            self.editing = Some(id);
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

        let (escape, delete, stress, reset) = ui.input(|i| {
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
            && let Some(id) = self.selected
        {
            self.blocks.retain(|block| block.id != id);
            self.selected = None;
        }
        if stress {
            self.spawn_stress();
        }
        if reset {
            self.reset_blocks();
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
        for index in 0..self.blocks.len() {
            let screen = self.view.rect_to_screen(self.blocks[index].rect);
            if !clip.intersects(screen) {
                continue;
            }
            let id = ui.scope_id().with(("block", self.blocks[index].id));
            let response = ui.interact(screen, id, Sense::click_and_drag());
            responses.push((index, response));
        }

        self.hovered = None;
        for (index, response) in responses {
            let id = self.blocks[index].id;
            if response.hovered() || response.dragged() {
                self.hovered = Some(id);
            }
            if response.dragged() && self.editing != Some(id) {
                let delta = response.drag_delta() / self.view.zoom;
                let moved = self.blocks[index].rect.translate(delta);
                self.blocks[index].rect = moved;
            }
            if response.clicked() {
                self.selected = Some(id);
            }
            if response.double_clicked() && self.blocks[index].kind == Kind::Text {
                self.selected = Some(id);
                self.editing = Some(id);
            }
        }
    }

    // ---- 绘制 ----

    fn paint_blocks(&self, ui: &egui::Ui, palette: &theme::Palette, clip: Rect) {
        let painter = ui.painter().with_clip_rect(clip);
        let zoom = self.view.zoom;

        for block in &self.blocks {
            let screen = self.view.rect_to_screen(block.rect);
            if !clip.intersects(screen) {
                continue;
            }

            let selected = self.selected == Some(block.id);
            let hovered = self.hovered == Some(block.id);
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
            let fill = if block.kind == Kind::Media {
                palette.media_fill
            } else {
                palette.block_fill
            };
            let radius = CornerRadius::same((theme::BLOCK_RADIUS * zoom).round() as u8);

            painter.rect(
                screen,
                radius,
                fill,
                Stroke::new(width, edge),
                StrokeKind::Inside,
            );

            match block.kind {
                Kind::Text => {
                    if self.editing != Some(block.id) {
                        paint_text(&painter, screen, &block.text, zoom, palette.text);
                    }
                }
                Kind::Image => {
                    if let Some(texture) = &self.texture {
                        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                        painter.image(texture.id(), screen, uv, Color32::WHITE);
                    }
                }
                Kind::Media => {
                    paint_play(&painter, screen, zoom, palette.muted);
                    let label = FontId::proportional((theme::SMALL_FONT * zoom).max(4.0));
                    let position = screen.center() + Vec2::new(0.0, 34.0 * zoom);
                    painter.text(
                        position,
                        Align2::CENTER_CENTER,
                        &block.text,
                        label,
                        palette.muted,
                    );
                }
            }
        }
    }

    fn edit_text(&mut self, ui: &mut egui::Ui, palette: &theme::Palette) {
        let Some(id) = self.editing else { return };
        let Some(index) = self.blocks.iter().position(|block| block.id == id) else {
            self.editing = None;
            return;
        };

        let zoom = self.view.zoom;
        let screen = self.view.rect_to_screen(self.blocks[index].rect);
        let font = FontId::proportional((theme::BLOCK_FONT * zoom).max(4.0));
        let margin = theme::BLOCK_PAD * zoom;
        let editor = egui::TextEdit::multiline(&mut self.blocks[index].text)
            .id(ui.scope_id().with(("editor", id)))
            .font(font)
            .text_color(palette.text)
            .background_color(palette.block_fill)
            .margin(egui::Margin::same(margin.round() as i8))
            .frame(egui::Frame::NONE)
            .desired_width((screen.width() - 2.0 * margin).max(8.0))
            .desired_rows(1);

        let response = ui.put(screen, editor);
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

        let mut hint = format!(
            "{}    {:.0}%   {:.0} fps",
            strings::BOARD_HINT,
            self.view.zoom * 100.0,
            self.fps
        );
        if let Some(note) = &self.image_note {
            hint.push_str("    ");
            hint.push_str(note);
        }

        let position = rect.left_bottom() + Vec2::new(theme::HINT_INSET, -theme::HINT_INSET);
        let font = FontId::proportional(theme::SMALL_FONT);
        ui.painter()
            .text(position, Align2::LEFT_BOTTOM, hint, font, palette.muted);
    }

    // ---- 内容 ----

    fn push_text(&mut self, min: Pos2, size: Vec2, text: &str) -> u64 {
        let id = self.next_block_id();
        self.blocks.push(Block {
            id,
            kind: Kind::Text,
            rect: Rect::from_min_size(min, size),
            text: text.to_owned(),
        });
        id
    }

    fn push_media(&mut self, min: Pos2, size: Vec2, label: &str) {
        let id = self.next_block_id();
        self.blocks.push(Block {
            id,
            kind: Kind::Media,
            rect: Rect::from_min_size(min, size),
            text: label.to_owned(),
        });
    }

    fn push_image(&mut self, min: Pos2, size: Vec2) {
        let id = self.next_block_id();
        self.blocks.push(Block {
            id,
            kind: Kind::Image,
            rect: Rect::from_min_size(min, size),
            text: String::new(),
        });
    }

    fn next_block_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn reset_blocks(&mut self) {
        self.blocks.clear();
        self.editing = None;
        self.selected = None;
        self.hovered = None;
        self.next_id = 1;
        self.push_text(
            Pos2::new(0.0, 0.0),
            Vec2::new(320.0, 140.0),
            "双击开始编辑：中文输入、选区、复制、撤销都在这里试。",
        );
        self.push_media(
            Pos2::new(0.0, 180.0),
            Vec2::new(320.0, 180.0),
            "视频块 · 待接入",
        );
        self.push_image(Pos2::new(360.0, 0.0), Vec2::new(420.0, 300.0));
    }

    /// 一千块铺满当前视口与下方，用来量拖动与缩放时的帧率。
    pub fn spawn_stress(&mut self) {
        let columns = 24;
        let start = self.view.to_world(Pos2::ZERO) - Vec2::splat(20.0);
        for index in 0..1000 {
            let x = start.x + (index % columns) as f32 * 70.0;
            let y = start.y + (index / columns) as f32 * 55.0;
            let min = Pos2::new(x, y);
            let size = Vec2::new(62.0, 47.0);
            if index % 3 == 0 {
                self.push_text(min, size, &format!("块 {index}"));
            } else {
                self.push_media(min, size, "占位");
            }
        }
    }

    // ---- 图片 ----

    fn load_image(&mut self, ctx: &egui::Context, path: Option<&std::path::Path>) {
        let (image, note) = match path {
            Some(path) => match load_color_image(path) {
                Ok(image) => (image, None),
                Err(note) => (sample_image(), Some(note)),
            },
            None => (sample_image(), None),
        };
        self.texture = Some(ctx.load_texture("board-image", image, egui::TextureOptions::LINEAR));
        self.image_note = note;
    }
}

/// 播放三角用图形画，不借字符——字符会落到彩色表情字体上，颜色和尺寸都不受控。
fn paint_play(painter: &egui::Painter, screen: Rect, zoom: f32, color: Color32) {
    let center = screen.center() - Vec2::new(0.0, 6.0 * zoom);
    let size = 14.0 * zoom;
    let points = vec![
        center + Vec2::new(-size * 0.5, -size),
        center + Vec2::new(-size * 0.5, size),
        center + Vec2::new(size, 0.0),
    ];
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
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

fn load_color_image(path: &std::path::Path) -> Result<egui::ColorImage, String> {
    let image = image::open(path).map_err(|error| format!("图片读取失败：{error}"))?;
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        rgba.as_raw(),
    ))
}

/// 程序生成的测试图：左半是四像素竖条，右半是棋盘，用来看缩放时的采样质量。
fn sample_image() -> egui::ColorImage {
    const W: usize = 320;
    const H: usize = 240;
    let mut pixels = Vec::with_capacity(W * H);
    for y in 0..H {
        for x in 0..W {
            let color = if x == 0 || y == 0 || x == W - 1 || y == H - 1 {
                Color32::WHITE
            } else if x < W / 2 {
                if (x / 4) % 2 == 0 {
                    Color32::from_rgb(236, 236, 236)
                } else {
                    Color32::from_rgb(24, 24, 24)
                }
            } else if (x / 32 + y / 32) % 2 == 0 {
                Color32::from_rgb(72, 108, 168)
            } else {
                Color32::from_rgb(30, 36, 46)
            };
            pixels.push(color);
        }
    }
    egui::ColorImage::new([W, H], pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> BoardApp {
        BoardApp::new(&egui::Context::default(), None)
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
}
