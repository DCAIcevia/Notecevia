#![forbid(unsafe_code)]

//! 画布：无限平面上的一堆块，加拖动、缩放与就地编辑。
//!
//! 绘制走屏幕坐标而不是整层变换：文本按缩放后的字号重新排版，放大后仍然清晰。
//! 代价是每帧要重排可见文字，所以绘制与交互都先按视口裁剪。

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use notecevia_core::board::{Document, Geometry};

use crate::{chrome, strings, theme};

/// 缩放范围。再小看不清，再大失去意义。
const MIN_ZOOM: f32 = 0.15;
const MAX_ZOOM: f32 = 6.0;
/// 新块的默认换行宽度。块的宽度就是这个：字到这儿换行。
const NEW_TEXT_WIDTH: f32 = 320.0;
/// 视图复位后的状态。
const DEFAULT_ORIGIN: Pos2 = Pos2::new(160.0, 140.0);
const DEFAULT_ZOOM: f32 = 1.0;

/// 鼠标落在块上时要做的事。位置决定动作，不摆按钮。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Grab {
    Move,
    Rotate,
    Horizontal { left: bool },
    Vertical { top: bool },
    Corner { left: bool, top: bool },
}

/// 由块本地坐标判断这次能做什么。本地坐标以块中心为原点、已经把旋转抵消掉。
fn grab_at(local: Vec2, half: Vec2) -> Option<Grab> {
    let x = local.x.abs();
    let y = local.y.abs();
    let left = local.x < 0.0;
    let top = local.y < 0.0;
    let near_x = x >= half.x - theme::GRAB_EDGE;
    let near_y = y >= half.y - theme::GRAB_EDGE;
    let outside_x = x > half.x + theme::GRAB_EDGE;
    let outside_y = y > half.y + theme::GRAB_EDGE;

    // 角外面那一圈是旋转，跟 PS 一样：贴着角往外拐就是转它。
    if outside_x && outside_y && x <= half.x + theme::GRAB_BAND && y <= half.y + theme::GRAB_BAND {
        return Some(Grab::Rotate);
    }
    if outside_x || outside_y {
        return None;
    }
    if near_x && near_y {
        return Some(Grab::Corner { left, top });
    }
    if near_x {
        return Some(Grab::Horizontal { left });
    }
    if near_y {
        return Some(Grab::Vertical { top });
    }
    Some(Grab::Move)
}

fn rotate(vector: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(
        vector.x * cos - vector.y * sin,
        vector.x * sin + vector.y * cos,
    )
}

/// 把屏幕坐标换成块本地坐标：以块中心为原点，并把旋转抵消掉。
fn local_point(pointer: Pos2, screen: Rect, rotation: f32) -> Vec2 {
    rotate(pointer - screen.center(), -rotation)
}

/// 两次指针位置相对块中心转过的角度，走最短的一边，避免跨过 ±π 时跳一整圈。
fn angle_around(previous: Pos2, current: Pos2, center: Pos2) -> f32 {
    let delta = (current - center).angle() - (previous - center).angle();
    if delta > core::f32::consts::PI {
        delta - core::f32::consts::TAU
    } else if delta < -core::f32::consts::PI {
        delta + core::f32::consts::TAU
    } else {
        delta
    }
}

/// 拉伸：在本地坐标里拖边或角，对面的边保持不动。
fn resized(geometry: Geometry, grab: Grab, delta: Vec2, zoom: f32) -> Geometry {
    let local = rotate(delta, -geometry.rotation) / zoom;
    let half = Vec2::new(geometry.width / 2.0, geometry.height / 2.0);
    let mut min = -half;
    let mut max = half;
    let mut dragged_max = false;
    let mut dragged_top = false;

    match grab {
        Grab::Horizontal { left } => {
            if left {
                min.x += local.x;
            } else {
                max.x += local.x;
                dragged_max = true;
            }
        }
        Grab::Vertical { top } => {
            if top {
                min.y += local.y;
            } else {
                max.y += local.y;
                dragged_max = true;
            }
        }
        Grab::Corner { left, top } => {
            if left {
                min.x += local.x;
            } else {
                max.x += local.x;
                dragged_max = true;
            }
            if top {
                min.y += local.y;
            } else {
                max.y += local.y;
                dragged_top = true;
            }
        }
        Grab::Move | Grab::Rotate => {}
    }

    // 拉到最小尺寸时顶住，不让它越拉越小。
    if max.x - min.x < theme::MIN_BLOCK_WIDTH {
        if dragged_max {
            max.x = min.x + theme::MIN_BLOCK_WIDTH;
        } else {
            min.x = max.x - theme::MIN_BLOCK_WIDTH;
        }
    }
    if max.y - min.y < theme::MIN_BLOCK_HEIGHT {
        if dragged_top {
            max.y = min.y + theme::MIN_BLOCK_HEIGHT;
        } else {
            min.y = max.y - theme::MIN_BLOCK_HEIGHT;
        }
    }

    let width = max.x - min.x;
    let height = max.y - min.y;
    let center = rotate(
        Vec2::new((min.x + max.x) / 2.0, (min.y + max.y) / 2.0),
        geometry.rotation,
    );
    let (center_x, center_y) = geometry.center();

    Geometry {
        x: center_x + center.x - width / 2.0,
        y: center_y + center.y - height / 2.0,
        width,
        height,
        rotation: geometry.rotation,
    }
}

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
    /// 刚进入编辑的那一块，下一帧要把焦点交给编辑框。
    pending_focus: Option<u64>,
    selected: Option<u64>,
    /// 正在拖的那块，以及拖的是移动、拉伸还是旋转。
    drag: Option<(u64, Grab)>,
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
            pending_focus: None,
            selected: None,
            drag: None,
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

    /// 选中第一块，不进入编辑。给命令行验证操作条用。
    pub fn select_first(&mut self) {
        if let Some(entry) = self.document.board.entries.first() {
            self.selected = Some(entry.id);
        }
    }

    /// 从第一块开始编辑。给命令行验证用，也因为以后要能从键盘直接进入编辑。
    pub fn begin_editing_first(&mut self) {
        if let Some(entry) = self.document.board.entries.first() {
            self.selected = Some(entry.id);
            self.editing = Some(entry.id);
            self.pending_focus = Some(entry.id);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let palette = theme::palette(ui.visuals().dark_mode);
        let window = ui.max_rect();
        let top = Rect::from_min_size(
            window.min,
            Vec2::new(window.width(), chrome::TOP_BAR_HEIGHT),
        );
        let rect = Rect::from_min_max(Pos2::new(window.min.x, top.max.y), window.max);
        let clip = ui.clip_rect().intersect(rect);

        // 先登记背景，再登记块：后登记的在上，块会拿走落在它身上的点击。
        let background = ui.interact(rect, ui.scope_id().with("canvas"), Sense::click_and_drag());
        self.apply_input(ui, &background, rect);

        ui.painter()
            .rect_filled(rect, CornerRadius::ZERO, palette.canvas);
        let state = if self.dirty {
            strings::UNSAVED.to_owned()
        } else {
            self.status.clone().unwrap_or_default()
        };
        chrome::top_bar(ui, &palette, top, &self.document.board.title, &state);

        self.interact_blocks(ui, clip);
        self.paint_blocks(ui, &palette, clip);
        self.edit_text(ui, &palette);
        self.style_toolbar(ui, &palette);
        self.paint_hud(ui, &palette, rect);
    }

    // ---- 输入 ----

    fn apply_input(&mut self, ui: &egui::Ui, background: &egui::Response, rect: Rect) {
        if background.dragged() {
            self.view.origin += background.drag_delta();
        }

        // 明确点一下才算写完：这一下既结束编辑，也把刚写的东西追认成一块。
        if background.clicked() {
            if self.editing.is_some() {
                self.commit_editing();
            } else {
                self.selected = None;
            }
        }

        if background.double_clicked()
            && let Some(pointer) = ui.input(|i| i.pointer.interact_pos())
        {
            let world = self.view.to_world(pointer);
            let block = self.document.add_text(Geometry::new(
                world.x,
                world.y,
                NEW_TEXT_WIDTH,
                theme::MIN_BLOCK_HEIGHT,
            ));
            // 这时候还没有真正的块：不给边框、不算选中，用户只是在这块地方开始写字。
            self.editing = Some(block);
            self.pending_focus = Some(block);
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
            self.commit_editing();
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

    /// 结束编辑：有字就追认成一块并选中它，没字就把这个空块丢掉。
    fn commit_editing(&mut self) {
        let Some(block) = self.editing.take() else {
            return;
        };
        self.pending_focus = None;
        if self.document.text_of(block).trim().is_empty() {
            self.document.remove_block(block);
            self.selected = None;
        } else {
            self.selected = Some(block);
            self.dirty = true;
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
        let pointer = ui.input(|input| input.pointer.hover_pos());
        let mut responses = Vec::new();

        for index in 0..self.document.board.entries.len() {
            let entry = &self.document.board.entries[index];
            let screen = self.view.rect_to_screen(rect_of(entry.geometry));
            if !clip.intersects(screen) {
                continue;
            }
            // 选中之后把可操作范围放大一圈：旋转手柄在块外面。
            let reach = if self.selected == Some(entry.id) {
                screen.expand(theme::GRAB_BAND)
            } else {
                screen
            };
            let id = ui.scope_id().with(("block", entry.id));
            responses.push((index, ui.interact(reach, id, Sense::click_and_drag())));
        }

        for (index, response) in responses {
            let block = self.document.board.entries[index].id;
            let geometry = self.document.board.entries[index].geometry;
            let screen = self.view.rect_to_screen(rect_of(geometry));
            let grab = pointer.and_then(|pointer| {
                grab_at(
                    local_point(pointer, screen, geometry.rotation),
                    screen.size() / 2.0,
                )
            });

            // 鼠标在哪决定干什么：字身上是拖动，边上拉伸，角外面旋转。
            let active = self.drag.map(|(dragged, grab)| (dragged == block, grab));
            if response.hovered() && self.editing != Some(block) {
                let icon = match grab {
                    Some(Grab::Rotate) => egui::CursorIcon::Crosshair,
                    Some(Grab::Horizontal { .. }) => egui::CursorIcon::ResizeHorizontal,
                    Some(Grab::Vertical { .. }) => egui::CursorIcon::ResizeVertical,
                    Some(Grab::Corner { left, top }) => {
                        if left == top {
                            egui::CursorIcon::ResizeNwSe
                        } else {
                            egui::CursorIcon::ResizeNeSw
                        }
                    }
                    Some(Grab::Move) | None => egui::CursorIcon::Grab,
                };
                ui.ctx().set_cursor_icon(icon);
            }

            if response.drag_started()
                && self.editing != Some(block)
                && let Some(grab) = grab
            {
                self.drag = Some((block, grab));
            }
            if let Some((true, grab)) = active {
                if grab == Grab::Move {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                }
                let delta = response.drag_delta();
                if delta != Vec2::ZERO {
                    let entry = &mut self.document.board.entries[index];
                    match grab {
                        Grab::Move => {
                            entry.geometry = translate(entry.geometry, delta / self.view.zoom)
                        }
                        Grab::Rotate => {
                            if let Some(pointer) = pointer {
                                entry.geometry.rotation +=
                                    angle_around(pointer - delta, pointer, screen.center());
                            }
                        }
                        Grab::Horizontal { .. } | Grab::Vertical { .. } | Grab::Corner { .. } => {
                            entry.geometry = resized(entry.geometry, grab, delta, self.view.zoom);
                        }
                    }
                    self.dirty = true;
                }
            }
            if response.drag_stopped() {
                self.drag = None;
            }
            if response.clicked() {
                self.commit_editing();
                self.selected = Some(block);
            }
            if response.double_clicked() {
                self.selected = Some(block);
                self.editing = Some(block);
                self.pending_focus = Some(block);
            }
        }
    }

    // ---- 绘制 ----

    fn paint_blocks(&mut self, ui: &egui::Ui, palette: &theme::Palette, clip: Rect) {
        let painter = ui.painter().with_clip_rect(clip);
        let zoom = self.view.zoom;

        for index in 0..self.document.board.entries.len() {
            let id = self.document.board.entries[index].id;
            let screen = self
                .view
                .rect_to_screen(rect_of(self.document.board.entries[index].geometry));
            if !clip.intersects(screen) {
                continue;
            }

            let editing = self.editing == Some(id);
            // 编辑框转不了，进编辑时先把这块摆正，写完再转回去。
            let rotation = if editing {
                0.0
            } else {
                self.document.board.entries[index].geometry.rotation
            };
            let size = self.block_size(id);
            let color = self.block_color(id, palette);
            let font = FontId::proportional((size * zoom).max(4.0));
            let galley = painter.layout(
                self.document.text_of(id).to_owned(),
                font,
                color,
                screen.width(),
            );

            // 块高至少要装得下文字：文字变多就长高，用户拉高了也不会被缩回去。
            let needed = galley.size().y.max(theme::MIN_BLOCK_HEIGHT);
            if self.document.board.entries[index].geometry.height < needed {
                self.document.board.entries[index].geometry.height = needed;
            }
            let height = self.document.board.entries[index].geometry.height;
            let block = Rect::from_min_size(screen.min, Vec2::new(screen.width(), height));

            // 块平时跟背景没有区别：不填充、不描边，只有字。确认并选中才画虚线框。
            if self.selected == Some(id) {
                dashed_outline(
                    &painter,
                    block,
                    rotation,
                    Stroke::new(theme::EDGE_WIDTH * zoom, palette.text),
                    zoom,
                );
            }

            if !editing {
                let text = egui::epaint::TextShape::new(screen.min, galley, color)
                    .with_angle_and_anchor(rotation, Align2::CENTER_CENTER);
                painter.add(egui::Shape::Text(text));
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
        let font = FontId::proportional((self.block_size(block) * zoom).max(4.0));
        let text_color = self.block_color(block, palette);

        // 编辑框和静态文字用同一个矩形、同样没有填充：进编辑的时候字不会跳。
        let inner = screen;
        let editor_id = ui.scope_id().with(("editor", block));

        let response = {
            let text = &mut self.document.text_mut(block).text;
            let editor = egui::TextEdit::multiline(text)
                .id(editor_id)
                .font(font.clone())
                .text_color(text_color)
                .background_color(Color32::TRANSPARENT)
                .margin(egui::Margin::ZERO)
                .frame(egui::Frame::NONE)
                .desired_width(inner.width())
                .desired_rows(1);
            ui.put(inner, editor)
        };

        // 焦点要等编辑框建好之后再要：它建起来时会因为「这次点击落在框外」而放弃焦点，
        // 先要就被立刻清掉。放到后面要，焦点留到下一帧，用户双击完就能直接打字。
        if self.pending_focus == Some(block) {
            ui.memory_mut(|memory| memory.request_focus(editor_id));
            self.pending_focus = None;
        }

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

        let right = format!("{:.0}%   {:.0} fps", self.view.zoom * 100.0, self.fps);
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

    /// 一块的字号。没设过就用主题默认值。
    fn block_size(&self, id: u64) -> f32 {
        self.document
            .texts
            .get(&id)
            .and_then(|block| block.size)
            .unwrap_or(theme::BLOCK_FONT)
    }

    /// 一块的颜色。没设过就跟随主题，浅色深色都好看。
    fn block_color(&self, id: u64, palette: &theme::Palette) -> Color32 {
        self.document
            .texts
            .get(&id)
            .and_then(|block| block.color)
            .map_or(palette.text, |[r, g, b]| Color32::from_rgb(r, g, b))
    }

    /// 选中一块时在它上方浮出格式操作条。编辑文字时不出现，免得抢焦点。
    fn style_toolbar(&mut self, ui: &mut egui::Ui, palette: &theme::Palette) {
        if self.editing.is_some() {
            return;
        }
        let Some(selected) = self.selected else {
            return;
        };
        let Some(entry) = self
            .document
            .board
            .entries
            .iter()
            .find(|entry| entry.id == selected)
        else {
            return;
        };
        let screen = rotated_bounds(
            self.view.rect_to_screen(rect_of(entry.geometry)),
            entry.geometry.rotation,
        );
        let Some(block) = self.document.texts.get_mut(&selected) else {
            return;
        };
        if chrome::style_toolbar(ui, palette, screen, block) {
            self.dirty = true;
        }
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

/// 虚线框。确认过的块才有，没有填充，只标边界。
fn dashed_outline(painter: &egui::Painter, rect: Rect, rotation: f32, stroke: Stroke, zoom: f32) {
    let rect = rect.shrink(stroke.width / 2.0);
    let center = rect.center();
    let corners: Vec<Pos2> = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ]
    .into_iter()
    .map(|corner| center + rotate(corner - center, rotation))
    .collect();
    for pair in corners.windows(2) {
        painter.extend(egui::Shape::dashed_line(
            pair,
            stroke,
            4.0 * zoom,
            3.0 * zoom,
        ));
    }
}

/// 旋转之后的包围盒，用来摆浮动的操作条。
fn rotated_bounds(rect: Rect, rotation: f32) -> Rect {
    let center = rect.center();
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
    .into_iter()
    .map(|corner| center + rotate(corner - center, rotation));
    Rect::from_points(&corners.collect::<Vec<_>>())
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

    fn frame(ctx: &egui::Context, app: &mut BoardApp, time: f64, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            time: Some(time),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 760.0))),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.ui(ui));
        // 没有渲染后端来消费纹理上传，测试里自己丢掉这些增量。
        output.textures_delta.clear();
    }

    fn click(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }

    /// 双击空白，开始写字。返回时处于编辑状态。
    fn begin_typing(ctx: &egui::Context, app: &mut BoardApp, pointer: Pos2) {
        frame(
            ctx,
            app,
            0.0,
            vec![egui::Event::PointerMoved(pointer), click(pointer, true)],
        );
        frame(ctx, app, 0.05, vec![click(pointer, false)]);
        frame(ctx, app, 0.10, vec![click(pointer, true)]);
        frame(ctx, app, 0.15, vec![click(pointer, false)]);
    }

    /// 在别处点一下：结束编辑并确认这一块。
    fn click_elsewhere(ctx: &egui::Context, app: &mut BoardApp, away: Pos2) {
        frame(
            ctx,
            app,
            0.30,
            vec![egui::Event::PointerMoved(away), click(away, true)],
        );
        frame(ctx, app, 0.35, vec![click(away, false)]);
    }

    #[test]
    fn double_click_makes_a_block_that_takes_typing() {
        let ctx = egui::Context::default();
        let mut app = BoardApp::new(Document::new("测试画板"));
        let pointer = Pos2::new(400.0, 300.0);

        begin_typing(&ctx, &mut app, pointer);
        assert_eq!(app.document().board.entries.len(), 1);

        frame(
            &ctx,
            &mut app,
            0.20,
            vec![egui::Event::Text("h".to_owned())],
        );
        assert_eq!(app.document().text_of(1), "h");
    }

    #[test]
    fn clicking_elsewhere_commits_the_block_and_selects_it() {
        let ctx = egui::Context::default();
        let mut app = BoardApp::new(Document::new("测试画板"));
        let pointer = Pos2::new(400.0, 300.0);

        begin_typing(&ctx, &mut app, pointer);
        // 还没确认：这块既没有虚线框，也不算被选中。
        assert!(app.selected.is_none());

        frame(
            &ctx,
            &mut app,
            0.20,
            vec![egui::Event::Text("你好".to_owned())],
        );
        click_elsewhere(&ctx, &mut app, Pos2::new(1000.0, 700.0));

        assert!(app.editing.is_none());
        assert_eq!(app.selected, Some(1));
        assert_eq!(app.document().text_of(1), "你好");
        assert_eq!(app.document().board.entries.len(), 1);
    }

    #[test]
    fn an_empty_block_disappears_when_committed() {
        let ctx = egui::Context::default();
        let mut app = BoardApp::new(Document::new("测试画板"));

        begin_typing(&ctx, &mut app, Pos2::new(400.0, 300.0));
        assert_eq!(app.document().board.entries.len(), 1);
        click_elsewhere(&ctx, &mut app, Pos2::new(1000.0, 700.0));

        assert!(app.document().board.entries.is_empty());
        assert!(app.selected.is_none());
    }

    #[test]
    fn the_mouse_position_decides_the_action() {
        let half = Vec2::new(100.0, 25.0);
        assert_eq!(grab_at(Vec2::ZERO, half), Some(Grab::Move));
        assert_eq!(
            grab_at(Vec2::new(99.0, 0.0), half),
            Some(Grab::Horizontal { left: false })
        );
        assert_eq!(
            grab_at(Vec2::new(-99.0, 0.0), half),
            Some(Grab::Horizontal { left: true })
        );
        assert_eq!(
            grab_at(Vec2::new(0.0, 24.0), half),
            Some(Grab::Vertical { top: false })
        );
        assert_eq!(
            grab_at(Vec2::new(100.0, 25.0), half),
            Some(Grab::Corner {
                left: false,
                top: false
            })
        );
        // 角外面那一圈是旋转
        assert_eq!(grab_at(Vec2::new(110.0, 35.0), half), Some(Grab::Rotate));
        // 更外面就什么都不做，交还给画布
        assert_eq!(grab_at(Vec2::new(300.0, 300.0), half), None);
    }

    #[test]
    fn resizing_keeps_the_opposite_edge_still() {
        let geometry = Geometry::new(0.0, 0.0, 100.0, 40.0);

        let wider = resized(
            geometry,
            Grab::Horizontal { left: false },
            Vec2::new(20.0, 0.0),
            1.0,
        );
        assert!((wider.width - 120.0).abs() < 0.001);
        assert!(wider.x.abs() < 0.001);

        let from_left = resized(
            geometry,
            Grab::Horizontal { left: true },
            Vec2::new(20.0, 0.0),
            1.0,
        );
        assert!((from_left.width - 80.0).abs() < 0.001);
        assert!((from_left.x - 20.0).abs() < 0.001);
    }

    #[test]
    fn rotating_a_quarter_turn_is_a_quarter_turn() {
        let rotated = rotate(Vec2::new(1.0, 0.0), core::f32::consts::FRAC_PI_2);
        assert!((rotated.x - 0.0).abs() < 0.001);
        assert!((rotated.y - 1.0).abs() < 0.001);
    }

    #[test]
    fn selection_survives_an_idle_frame() {
        let ctx = egui::Context::default();
        let mut document = Document::new("测试画板");
        let id = document.add_text(Geometry::new(0.0, 0.0, 200.0, 60.0));
        document.text_mut(id).text = "一行字".to_owned();
        let mut app = BoardApp::new(document);

        app.select_first();
        assert_eq!(app.selected, Some(id));
        frame(&ctx, &mut app, 0.0, Vec::new());
        assert_eq!(app.selected, Some(id));
    }

    #[test]
    fn geometry_survives_a_translation() {
        let geometry = Geometry::new(10.0, 20.0, 30.0, 40.0);
        let moved = translate(geometry, Vec2::new(5.0, -5.0));
        assert_eq!(moved, Geometry::new(15.0, 15.0, 30.0, 40.0));
    }
}
