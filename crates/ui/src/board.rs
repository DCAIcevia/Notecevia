#![forbid(unsafe_code)]

//! 画布：无限平面上的一堆块，加拖动、缩放、就地编辑与撤销。
//!
//! 绘制走屏幕坐标而不是整层变换：文本按缩放后的字号重新排版，放大后仍然清晰。
//! 代价是每帧要重排可见文字，所以绘制与交互都先按视口裁剪。
//!
//! 所有修改文档的路径都经过 `edit` 或 `commit_change`：前者直接包一次改动，后者把
//! 拖动、打字这类「先改、后结束」的过程收成一步撤销。

use std::collections::HashSet;

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use notecevia_core::{
    BlockKind, Body, Document, Entry, Geometry, HighlightBlock, History, ImageBlock, ImageFit,
    MediaBlock, ShapeKind, TextBlock,
};

use crate::media::{ImageCache, MediaSource};
use crate::{chrome, strings, theme};

/// 缩放范围。再小看不清，再大失去意义。
const MIN_ZOOM: f32 = 0.15;
const MAX_ZOOM: f32 = 6.0;
/// 新建文字块的默认大小。
const NEW_TEXT_WIDTH: f32 = 320.0;
const NEW_TEXT_HEIGHT: f32 = 60.0;
const NEW_TABLE_SIZE: Vec2 = Vec2::new(320.0, 180.0);
const NEW_SHAPE_SIZE: Vec2 = Vec2::new(160.0, 120.0);
const NEW_HIGHLIGHT_SIZE: Vec2 = Vec2::new(200.0, 80.0);
/// 视图复位后的状态。
const DEFAULT_ORIGIN: Pos2 = Pos2::new(160.0, 140.0);
const DEFAULT_ZOOM: f32 = 1.0;
/// 普通状态提示停留多久（秒）。
const STATUS_TTL: f64 = 3.0;

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

fn cursor_for(grab: Option<Grab>, locked: bool) -> egui::CursorIcon {
    if locked {
        return egui::CursorIcon::Default;
    }
    match grab {
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
    }
}

/// 视口：世界坐标与屏幕坐标之间的换算。
#[derive(Clone, Copy)]
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

    fn to_screen(self, point: Pos2) -> Pos2 {
        self.origin + point.to_vec2() * self.zoom
    }

    fn to_world(self, point: Pos2) -> Pos2 {
        ((point - self.origin) / self.zoom).to_pos2()
    }

    fn rect_to_screen(self, rect: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(rect.min), self.to_screen(rect.max))
    }
}

/// 把一块世界矩形放进画布：四周留边，缩放夹在允许范围内。
fn fit_view(bounds: Rect, canvas: Rect) -> View {
    const PADDING: f32 = 32.0;
    let width = bounds.width().max(1.0);
    let height = bounds.height().max(1.0);
    let available_x = (canvas.width() - PADDING * 2.0).max(1.0);
    let available_y = (canvas.height() - PADDING * 2.0).max(1.0);
    let zoom = (available_x / width)
        .min(available_y / height)
        .clamp(MIN_ZOOM, MAX_ZOOM);
    let origin = canvas.center() - bounds.center().to_vec2() * zoom;
    View { origin, zoom }
}

/// 正在编辑的东西。表格和文字一样是就地编辑，但表格多一个当前单元格。
#[derive(Clone, Debug)]
enum Editing {
    Text(u64),
    Table {
        id: u64,
        cell: Option<(u16, u16)>,
        /// 单元格的草稿：按 Enter 或失焦才写回文档。
        draft: String,
    },
}

/// 正在拖的块，以及拖动开始前的快照，松手时把整段拖动收成一步撤销。
struct DragState {
    id: u64,
    grab: Grab,
    before: Document,
}

/// 空白处拉出来的选框，坐标是世界坐标。
struct Marquee {
    start: Pos2,
    current: Pos2,
    additive: bool,
}

/// 一条状态。错误常驻到被下一条替换；普通状态几秒后自己消失。
struct Notice {
    text: String,
    error: bool,
    /// 第一次显示的时间，由 `ui` 从 egui 的 input time 取，不引时钟。
    born: Option<f64>,
}

/// 画布界面。数据在 Document 里，这里只管怎么显示、怎么改。
pub struct BoardApp {
    view: View,
    document: Document,
    /// 上次保存时的样子。和当前文档比较就是「有没有改动」，撤销回保存点也会变干净。
    saved: Document,
    history: History,
    media: Box<dyn MediaSource>,
    images: ImageCache,
    editing: Option<Editing>,
    /// 编辑框刚建好，下一帧把焦点交给它。
    request_focus: bool,
    selected: Vec<u64>,
    drag: Option<DragState>,
    marquee: Option<Marquee>,
    /// 进入文字编辑前 / 新建文字块前的快照，结束时收成一步撤销。
    edit_before: Option<Document>,
    /// 系统字体，枚举要几百毫秒，所以第一次用到才建。
    fonts: Option<crate::fonts::FontLibrary>,
    installed_fonts: HashSet<String>,
    font_filter: String,
    status: Option<Notice>,
    /// 双击视频/音频块时记下哈希，等 app 取走去开系统播放器。
    media_request: Option<String>,
    /// 顶栏点了「全部画板」，等 app 取走。
    home_request: bool,
    /// 顶栏点了「导出图片」，等 app 截完图取走。
    image_export_request: bool,
    fps: f32,
    /// 上一帧画布的面积，用来把新块放在视口中心。
    canvas: Rect,
    /// 为缩略图临时改成「缩到全貌」的视图时，这里存原来的视图。
    thumbnail_view: Option<View>,
}

impl BoardApp {
    pub fn new(document: Document, media: Box<dyn MediaSource>) -> Self {
        Self {
            view: View::restored(),
            saved: document.clone(),
            document,
            history: History::new(),
            media,
            images: ImageCache::default(),
            editing: None,
            request_focus: false,
            selected: Vec::new(),
            drag: None,
            marquee: None,
            edit_before: None,
            fonts: None,
            installed_fonts: HashSet::new(),
            font_filter: String::new(),
            status: None,
            media_request: None,
            home_request: false,
            image_export_request: false,
            fps: 0.0,
            canvas: Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 760.0)),
            thumbnail_view: None,
        }
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    /// 存过盘之后调用：把当前文档记成保存点，未保存标记随之消失。
    pub fn mark_saved(&mut self) {
        self.saved = self.document.clone();
    }

    pub fn is_dirty(&self) -> bool {
        self.document != self.saved
    }

    /// 相对保存快照，正文有新增或内容变化的块编号。
    ///
    /// 只比较正文：纯几何、层级、锁定/隐藏的变化不算，删除的块也不在这里
    /// （那块正文已经不在文档里，由保存后的孤儿回收处理）。给增量保存用。
    pub fn changed_bodies(&self) -> Vec<u64> {
        self.document
            .bodies
            .iter()
            .filter(|(id, body)| self.saved.bodies.get(*id) != Some(*body))
            .map(|(id, _)| *id)
            .collect()
    }

    /// 在顶栏右侧显示一条普通状态，几秒后自动消失。
    ///
    /// 传进来的如果是 `strings` 里那些失败文案（以 save_failed 之类开头），会被当作
    /// 错误处理：错误优先于「未保存」，不会被盖掉。
    pub fn set_status(&mut self, status: impl Into<String>) {
        let text = status.into();
        let error = looks_like_error(&text);
        self.status = Some(Notice {
            text,
            error,
            born: None,
        });
    }

    /// 明确记录一条错误。app 只用 set_status 也可以，这条是给语义清楚的调用方。
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.status = Some(Notice {
            text: error.into(),
            error: true,
            born: None,
        });
    }

    /// 换一张画板：视图、选中、编辑、撤销栈全部回到初始。
    pub fn replace_document(&mut self, document: Document) {
        self.document = document;
        self.saved = self.document.clone();
        self.history.clear();
        self.view = View::restored();
        self.editing = None;
        self.request_focus = false;
        self.selected.clear();
        self.drag = None;
        self.marquee = None;
        self.edit_before = None;
        self.images.clear();
        self.status = None;
        self.media_request = None;
        self.home_request = false;
        self.image_export_request = false;
        self.thumbnail_view = None;
    }

    /// 拖入一份媒体：按视口中心放一块，返回时这一块已被选中。
    pub fn insert_media(&mut self, kind: BlockKind, media: String, name: String, bytes: u64) {
        if !kind.is_media() {
            return;
        }
        let size = media_size(kind);
        let centre = self.view.to_world(self.canvas.center());
        let id = self.edit(|document| {
            let geometry = Geometry::new(
                centre.x - size.x / 2.0,
                centre.y - size.y / 2.0,
                size.x,
                size.y,
            );
            let z = document.max_z() + 1;
            let id = document.add_block(kind, geometry);
            if let Some(entry) = document.entry_mut(id) {
                entry.z = z;
            }
            match document.body_mut(id) {
                Some(body) if kind == BlockKind::Image => {
                    *body = Body::Image(ImageBlock {
                        media,
                        name,
                        fit: ImageFit::Cover,
                    });
                }
                Some(body) => {
                    *body = Body::Media(MediaBlock { media, name, bytes });
                }
                None => {}
            }
            id
        });
        self.selected = vec![id];
    }

    /// 双击过视频/音频块时，app 在这里取走要打开的媒体哈希。
    pub fn take_media_request(&mut self) -> Option<String> {
        self.media_request.take()
    }

    /// 顶栏点过「全部画板」时返回 true，取走时清掉。
    pub fn take_home_request(&mut self) -> bool {
        let requested = self.home_request;
        self.home_request = false;
        requested
    }

    /// 顶栏点过「导出图片」时返回 true，取走时清掉。截图与保存由 app 做。
    pub fn take_image_export_request(&mut self) -> bool {
        let requested = self.image_export_request;
        self.image_export_request = false;
        requested
    }

    /// 最近一帧画布区域在屏幕坐标里的矩形（不含顶栏），app 用它裁剪截图。
    pub fn canvas_rect(&self) -> egui::Rect {
        self.canvas
    }

    /// 为缩略图把视图缩到内容全貌。返回是否真的改了视图（没有可见块时不动）。
    /// 调用之后要再画一帧让 app 截图，然后调 `end_thumbnail_view` 还原。
    pub fn begin_thumbnail_view(&mut self) -> bool {
        if self.thumbnail_view.is_some() {
            return false;
        }
        let Some(bounds) = self.content_bounds() else {
            return false;
        };
        self.thumbnail_view = Some(self.view);
        self.view = fit_view(bounds, self.canvas);
        true
    }

    /// 把缩略图那次临时缩放的视图还原。
    pub fn end_thumbnail_view(&mut self) {
        if let Some(view) = self.thumbnail_view.take() {
            self.view = view;
        }
    }

    /// 所有可见块合起来的外接矩形，世界坐标。
    fn content_bounds(&self) -> Option<Rect> {
        let mut bounds: Option<Rect> = None;
        for entry in &self.document.board.entries {
            if !entry.visible {
                continue;
            }
            let geometry = entry.geometry;
            let rect = Rect::from_min_size(
                Pos2::new(geometry.x, geometry.y),
                Vec2::new(geometry.width, geometry.height),
            );
            bounds = Some(match bounds {
                None => rect,
                Some(previous) => previous.union(rect),
            });
        }
        bounds
    }

    /// 选中第一块，不进入编辑。给命令行验证操作条用。
    pub fn select_first(&mut self) {
        if let Some(entry) = self.document.board.entries.first() {
            self.selected = vec![entry.id];
        }
    }

    /// 从第一块开始编辑。给命令行验证用。
    pub fn begin_editing_first(&mut self) {
        let Some((id, kind)) = self
            .document
            .board
            .entries
            .first()
            .map(|entry| (entry.id, entry.kind))
        else {
            return;
        };
        self.selected = vec![id];
        match kind {
            BlockKind::Text => self.begin_text_edit(id),
            BlockKind::Table => self.begin_table_edit(id, None),
            _ => {}
        }
    }

    /// 一千块铺满当前视口与下方，用来量拖动与缩放时的帧率。
    pub fn spawn_stress(&mut self) {
        let start = self.view.to_world(Pos2::ZERO) - Vec2::splat(20.0);
        self.edit(|document| {
            for index in 0..1000 {
                let x = start.x + (index % 24) as f32 * 70.0;
                let y = start.y + (index / 24) as f32 * 55.0;
                let id = document.add_text(Geometry::new(x, y, 62.0, 47.0));
                document.text_mut(id).text = format!("块 {index}");
            }
        });
    }

    /// 开发检查用：给第一块文本末尾加一个字符，走正常的撤销与未保存路径。
    ///
    /// 没有文本块时什么都不做。用来手动验证增量保存是否只写这一块正文。
    pub fn nudge_first_text(&mut self) {
        let first = self
            .document
            .board
            .entries
            .iter()
            .find(|entry| entry.kind == BlockKind::Text)
            .map(|entry| entry.id);
        let Some(id) = first else {
            return;
        };
        self.edit(|document| {
            if let Some(Body::Text(text)) = document.body_mut(id) {
                text.text.push('x');
            }
        });
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let palette = theme::palette(ui.visuals().dark_mode);
        let window = ui.max_rect();
        let top = Rect::from_min_size(
            window.min,
            Vec2::new(window.width(), chrome::TOP_BAR_HEIGHT),
        );
        let canvas = Rect::from_min_max(Pos2::new(window.min.x, top.max.y), window.max);
        self.canvas = canvas;
        let clip = ui.clip_rect().intersect(canvas);
        let now = ui.input(|input| input.time);
        self.touch_status(now);

        // 先登记背景，再登记块：后登记的在上，块会拿走落在它身上的点击。
        let background = ui.interact(
            canvas,
            ui.scope_id().with("canvas"),
            Sense::click_and_drag(),
        );
        self.handle_background(ui, &background, canvas);
        ui.painter()
            .rect_filled(canvas, CornerRadius::ZERO, palette.canvas);

        let (state, error) = self.status_text(now);
        let top_output =
            chrome::top_bar(ui, &palette, top, &self.document.board.title, &state, error);
        if top_output.home {
            self.home_request = true;
        }
        if top_output.export_image {
            self.image_export_request = true;
        }
        if let Some(kind) = top_output.new_kind {
            self.insert_new(kind);
        }

        self.handle_keys(ui);
        self.ensure_fonts(ui.ctx());
        self.interact_blocks(ui, clip);
        self.paint_blocks(ui, &palette, clip);
        self.paint_marquee(ui, &palette, clip);
        self.paint_hud(ui, &palette, canvas);
        self.edit_text(ui, &palette);
        self.edit_table(ui, &palette);
        self.selection_toolbar(ui, &palette, canvas);
        self.schedule_status_repaint(ui, now);
    }

    // ---- 状态 ----

    fn touch_status(&mut self, now: f64) {
        if let Some(notice) = &mut self.status
            && !notice.error
            && notice.born.is_none()
        {
            notice.born = Some(now);
        }
    }

    fn status_text(&self, now: f64) -> (String, bool) {
        // 优先级：错误 > 未保存 > 普通状态。保存失败的提示不能被「未保存」盖掉。
        if let Some(notice) = &self.status
            && notice.error
        {
            return (notice.text.clone(), true);
        }
        if self.is_dirty() {
            return (strings::UNSAVED.to_owned(), false);
        }
        if let Some(notice) = &self.status
            && notice.born.is_none_or(|born| now - born < STATUS_TTL)
        {
            return (notice.text.clone(), false);
        }
        (String::new(), false)
    }

    fn schedule_status_repaint(&self, ui: &egui::Ui, now: f64) {
        let Some(notice) = &self.status else {
            return;
        };
        if notice.error {
            return;
        }
        if let Some(born) = notice.born {
            let left = STATUS_TTL - (now - born);
            if left > 0.0 {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(left));
            }
        }
    }

    // ---- 输入 ----

    fn handle_background(&mut self, ui: &egui::Ui, background: &egui::Response, canvas: Rect) {
        // 中键拖动平移。左键留给选框，滚轮也能平移。
        if background.dragged_by(egui::PointerButton::Middle) {
            self.view.origin += background.drag_delta();
        }

        if ui.rect_contains_pointer(canvas) {
            let (scroll, zoom_delta, pointer) = ui.input(|input| {
                (
                    input.smooth_scroll_delta,
                    input.zoom_delta(),
                    input.pointer.hover_pos(),
                )
            });
            if let Some(pointer) = pointer {
                if zoom_delta != 1.0 {
                    self.zoom_at(pointer, zoom_delta);
                } else if scroll != Vec2::ZERO {
                    self.view.origin += scroll;
                }
            }
        }

        if background.drag_started_by(egui::PointerButton::Primary)
            && self.editing.is_none()
            && let Some(start) = ui.input(|input| input.pointer.press_origin())
        {
            // 从按下的那一点起框：egui 要等移动超过阈值才报 drag_started，用当前指针会丢掉开头。
            let additive = ui.input(|input| input.modifiers.shift);
            let world = self.view.to_world(start);
            self.marquee = Some(Marquee {
                start: world,
                current: world,
                additive,
            });
            if !additive {
                self.selected.clear();
            }
        }

        if let Some(marquee) = &mut self.marquee
            && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
        {
            marquee.current = self.view.to_world(pointer);
        }

        if background.drag_stopped_by(egui::PointerButton::Primary)
            && let Some(marquee) = self.marquee.take()
        {
            self.select_in_marquee(marquee);
        }

        if background.clicked() {
            if self.editing.is_some() {
                self.commit_editing();
            } else {
                self.selected.clear();
            }
        }

        if background.double_clicked()
            && let Some(pointer) = ui.input(|input| input.pointer.interact_pos())
        {
            let world = self.view.to_world(pointer);
            self.begin_new_text(world);
        }
    }

    fn handle_keys(&mut self, ui: &egui::Ui) {
        let typing = ui.ctx().egui_wants_keyboard_input();
        let (escape, delete, select_all, duplicate, undo, redo, stress, restore_view) =
            ui.input(|input| {
                let command = input.modifiers.command;
                let shift = input.modifiers.shift;
                let undo = command && !shift && input.key_pressed(egui::Key::Z);
                let redo = (command && shift && input.key_pressed(egui::Key::Z))
                    || (command && input.key_pressed(egui::Key::Y));
                (
                    input.key_pressed(egui::Key::Escape),
                    input.key_pressed(egui::Key::Delete),
                    command && input.key_pressed(egui::Key::A),
                    command && input.key_pressed(egui::Key::D),
                    undo,
                    redo,
                    input.key_pressed(egui::Key::F2),
                    input.key_pressed(egui::Key::F3),
                )
            });

        if escape {
            if self.editing.is_some() {
                self.commit_editing();
            } else {
                self.selected.clear();
            }
        }
        if stress {
            self.spawn_stress();
        }
        if restore_view {
            self.view = View::restored();
        }

        // 编辑框拿着键盘时，这些键归输入框自己（比如它自己的撤销）。
        if typing {
            return;
        }
        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        if delete {
            self.delete_selected();
        }
        if select_all {
            self.selected = self
                .document
                .board
                .entries
                .iter()
                .map(|entry| entry.id)
                .collect();
        }
        if duplicate {
            self.duplicate_selected();
        }
    }

    fn zoom_at(&mut self, pointer: Pos2, factor: f32) {
        let world = self.view.to_world(pointer);
        self.view.zoom = (self.view.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.view.origin = pointer - world.to_vec2() * self.view.zoom;
    }

    fn select_in_marquee(&mut self, marquee: Marquee) {
        let area = Rect::from_two_pos(marquee.start, marquee.current);
        let hits: Vec<u64> = self
            .document
            .board
            .entries
            .iter()
            .filter(|entry| entry.visible && rect_of(entry.geometry).intersects(area))
            .map(|entry| entry.id)
            .collect();
        if marquee.additive {
            for id in hits {
                if !self.selected.contains(&id) {
                    self.selected.push(id);
                }
            }
        } else {
            self.selected = hits;
        }
    }

    // ---- 交互 ----

    fn interact_blocks(&mut self, ui: &egui::Ui, clip: Rect) {
        let pointer = ui.input(|input| input.pointer.hover_pos());
        let shift = ui.input(|input| input.modifiers.shift);
        let editing = self.editing_block();
        let order = self.draw_order();
        let mut responses = Vec::new();

        for index in order {
            let entry = self.document.board.entries[index].clone();
            if !entry.visible {
                continue;
            }
            let screen = self.view.rect_to_screen(rect_of(entry.geometry));
            if !clip.intersects(screen) {
                continue;
            }
            // 选中之后把可操作范围放大一圈：旋转手柄在块外面。
            let reach = if self.is_selected(entry.id) {
                screen.expand(theme::GRAB_BAND)
            } else {
                screen
            };
            let id = ui.scope_id().with(("block", entry.id));
            responses.push((index, ui.interact(reach, id, Sense::click_and_drag())));
        }

        for (index, response) in responses {
            let entry = self.document.board.entries[index].clone();
            let id = entry.id;
            let screen = self.view.rect_to_screen(rect_of(entry.geometry));
            let grab = pointer.and_then(|pointer| {
                grab_at(
                    local_point(pointer, screen, entry.geometry.rotation),
                    screen.size() / 2.0,
                )
            });

            if response.hovered() && editing != Some(id) {
                ui.ctx().set_cursor_icon(cursor_for(grab, entry.locked));
            }

            // 拖动按下的那一点决定动作：等 drag_started 报上来时指针可能已经离开小块了。
            if response.drag_started()
                && editing != Some(id)
                && !entry.locked
                && let Some(origin) = ui.input(|input| input.pointer.press_origin())
                && let Some(grab) = grab_at(
                    local_point(origin, screen, entry.geometry.rotation),
                    screen.size() / 2.0,
                )
            {
                self.commit_editing();
                if !self.is_selected(id) {
                    self.selected = vec![id];
                }
                let single = self.selected.len() == 1;
                if grab == Grab::Move || single {
                    self.drag = Some(DragState {
                        id,
                        grab,
                        before: self.document.clone(),
                    });
                }
            }

            let dragging_this = self.drag.as_ref().is_some_and(|drag| drag.id == id);
            if dragging_this {
                self.drag_blocks(ui, &response, id, &entry);
            }
            if response.drag_stopped()
                && dragging_this
                && let Some(drag) = self.drag.take()
            {
                self.commit_change(drag.before);
            }

            if response.clicked() {
                let editing_table = matches!(self.editing, Some(Editing::Table { id: current, .. }) if current == id);
                if !editing_table {
                    self.commit_editing();
                    if shift {
                        self.toggle_selection(id);
                    } else {
                        self.select_only(id);
                    }
                }
            }

            if response.double_clicked() && !entry.locked {
                self.begin_block_edit(id, entry.kind, pointer);
            }
        }
    }

    fn drag_blocks(&mut self, ui: &egui::Ui, response: &egui::Response, id: u64, entry: &Entry) {
        let Some(drag) = &self.drag else {
            return;
        };
        let grab = drag.grab;
        let delta = response.drag_delta();
        if delta == Vec2::ZERO {
            return;
        }

        if grab == Grab::Move {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            let world = delta / self.view.zoom;
            let targets: Vec<u64> = if self.is_selected(id) {
                self.selected.clone()
            } else {
                vec![id]
            };
            for target in targets {
                if let Some(entry) = self.document.entry_mut(target)
                    && !entry.locked
                {
                    entry.geometry.x += world.x;
                    entry.geometry.y += world.y;
                }
            }
            return;
        }

        let Some(pointer) = ui.input(|input| input.pointer.hover_pos()) else {
            return;
        };
        let screen = self.view.rect_to_screen(rect_of(entry.geometry));
        let Some(target) = self.document.entry_mut(id) else {
            return;
        };
        match grab {
            Grab::Rotate => {
                target.geometry.rotation += angle_around(pointer - delta, pointer, screen.center());
            }
            Grab::Horizontal { .. } | Grab::Vertical { .. } | Grab::Corner { .. } => {
                target.geometry = resized(target.geometry, grab, delta, self.view.zoom);
            }
            Grab::Move => {}
        }
    }

    fn begin_block_edit(&mut self, id: u64, kind: BlockKind, pointer: Option<Pos2>) {
        self.commit_editing();
        match kind {
            BlockKind::Text => self.begin_text_edit(id),
            BlockKind::Table => self.begin_table_edit(id, pointer),
            BlockKind::Video | BlockKind::Audio => {
                if let Some(Body::Media(media)) = self.document.body(id)
                    && !media.media.is_empty()
                {
                    self.media_request = Some(media.media.clone());
                }
            }
            BlockKind::Image | BlockKind::Shape | BlockKind::Highlight => {}
        }
    }

    fn begin_text_edit(&mut self, id: u64) {
        if !matches!(self.document.body(id), Some(Body::Text(_))) {
            return;
        }
        self.edit_before = Some(self.document.clone());
        self.editing = Some(Editing::Text(id));
        self.request_focus = true;
        self.selected = vec![id];
    }

    fn begin_new_text(&mut self, world: Pos2) {
        let before = self.document.clone();
        let z = self.document.max_z() + 1;
        let id = self.document.add_block(
            BlockKind::Text,
            Geometry::new(world.x, world.y, NEW_TEXT_WIDTH, NEW_TEXT_HEIGHT),
        );
        if let Some(entry) = self.document.entry_mut(id) {
            entry.z = z;
        }
        self.editing = Some(Editing::Text(id));
        self.edit_before = Some(before);
        self.request_focus = true;
        self.selected.clear();
    }

    fn begin_table_edit(&mut self, id: u64, pointer: Option<Pos2>) {
        if !matches!(self.document.body(id), Some(Body::Table(_))) {
            return;
        }
        let cell = pointer.and_then(|pointer| self.table_cell_at(id, pointer));
        let draft = cell.map_or_else(String::new, |(row, column)| {
            self.table_cell_text(id, row, column)
        });
        self.editing = Some(Editing::Table { id, cell, draft });
        self.request_focus = true;
        self.selected = vec![id];
    }

    fn table_cell_at(&self, id: u64, pointer: Pos2) -> Option<(u16, u16)> {
        let entry = self.document.entry(id)?;
        let Some(Body::Table(table)) = self.document.body(id) else {
            return None;
        };
        let screen = self.view.rect_to_screen(rect_of(entry.geometry));
        if !screen.contains(pointer) {
            return None;
        }
        let local = pointer - screen.min;
        let column = (local.x / screen.width() * f32::from(table.cols))
            .floor()
            .clamp(0.0, f32::from(table.cols.saturating_sub(1)));
        let row = (local.y / screen.height() * f32::from(table.rows))
            .floor()
            .clamp(0.0, f32::from(table.rows.saturating_sub(1)));
        Some((row as u16, column as u16))
    }

    fn table_cell_text(&self, id: u64, row: u16, column: u16) -> String {
        match self.document.body(id) {
            Some(Body::Table(table)) => table.cell(row, column).to_owned(),
            _ => String::new(),
        }
    }

    fn commit_table_cell(&mut self) {
        let Some(Editing::Table { id, cell, draft }) = &self.editing else {
            return;
        };
        let (id, cell, draft) = (*id, *cell, draft.clone());
        let Some((row, column)) = cell else {
            return;
        };
        if let Some(Editing::Table { cell, .. }) = &mut self.editing {
            *cell = None;
        }
        if self.table_cell_text(id, row, column) == draft {
            return;
        }
        self.edit(|document| {
            if let Some(Body::Table(table)) = document.body_mut(id) {
                table.set_cell(row, column, draft);
            }
        });
    }

    fn commit_editing(&mut self) {
        let Some(editing) = self.editing.take() else {
            return;
        };
        self.request_focus = false;
        match editing {
            Editing::Text(id) => {
                let before = self.edit_before.take();
                if self.document.text_of(id).trim().is_empty() {
                    self.document.remove_block(id);
                    self.selected.retain(|selected| *selected != id);
                } else {
                    self.selected = vec![id];
                }
                if let Some(before) = before {
                    self.commit_change(before);
                }
            }
            Editing::Table { id, cell, draft } => {
                self.editing = Some(Editing::Table { id, cell, draft });
                self.commit_table_cell();
                self.editing = None;
                self.edit_before = None;
            }
        }
    }

    // ---- 撤销 ----

    /// 统一改文档的入口：改完了才记撤销，改不动就不记。
    fn edit<R>(&mut self, change: impl FnOnce(&mut Document) -> R) -> R {
        let before = self.document.clone();
        let result = change(&mut self.document);
        if self.document != before {
            let after = std::mem::replace(&mut self.document, before);
            self.history
                .apply(&mut self.document, |document| *document = after);
        }
        result
    }

    /// 把一段已经直接改完的过程（拖动、打字）收成一步撤销。
    fn commit_change(&mut self, before: Document) {
        if self.document == before {
            return;
        }
        let after = std::mem::replace(&mut self.document, before);
        self.history
            .apply(&mut self.document, |document| *document = after);
    }

    fn undo(&mut self) {
        self.commit_editing();
        if self.history.undo(&mut self.document) {
            self.after_time_travel();
            self.set_status(strings::UNDONE);
        }
    }

    fn redo(&mut self) {
        self.commit_editing();
        if self.history.redo(&mut self.document) {
            self.after_time_travel();
            self.set_status(strings::REDONE);
        }
    }

    fn after_time_travel(&mut self) {
        self.editing = None;
        self.request_focus = false;
        self.edit_before = None;
        self.drag = None;
        self.marquee = None;
        self.selected
            .retain(|id| self.document.entry(*id).is_some());
    }

    fn delete_selected(&mut self) {
        if self.selected.is_empty() {
            return;
        }
        let selected = std::mem::take(&mut self.selected);
        self.edit(|document| {
            for id in &selected {
                document.remove_block(*id);
            }
        });
        self.set_status(strings::BLOCK_DELETED);
    }

    fn duplicate_selected(&mut self) {
        let selected = self.selected.clone();
        if selected.is_empty() {
            return;
        }
        let copies = self.edit(|document| {
            let mut copies = Vec::new();
            for id in &selected {
                if let Some(copy) = document.duplicate_block(*id) {
                    copies.push(copy);
                }
            }
            copies
        });
        if !copies.is_empty() {
            self.selected = copies;
            self.set_status(strings::COPIED);
        }
    }

    // ---- 新建与工具条 ----

    fn insert_new(&mut self, kind: BlockKind) {
        let centre = self.view.to_world(self.canvas.center());
        match kind {
            BlockKind::Text => {
                let world = centre - Vec2::new(NEW_TEXT_WIDTH, NEW_TEXT_HEIGHT) / 2.0;
                self.begin_new_text(world);
            }
            BlockKind::Table => self.insert_block(BlockKind::Table, NEW_TABLE_SIZE, centre),
            BlockKind::Shape => self.insert_block(BlockKind::Shape, NEW_SHAPE_SIZE, centre),
            BlockKind::Highlight => {
                self.insert_block(BlockKind::Highlight, NEW_HIGHLIGHT_SIZE, centre);
            }
            BlockKind::Image | BlockKind::Video | BlockKind::Audio => {}
        }
    }

    fn insert_block(&mut self, kind: BlockKind, size: Vec2, centre: Pos2) {
        let geometry = Geometry::new(
            centre.x - size.x / 2.0,
            centre.y - size.y / 2.0,
            size.x,
            size.y,
        );
        let id = self.edit(|document| {
            let z = document.max_z() + 1;
            let id = document.add_block(kind, geometry);
            if let Some(entry) = document.entry_mut(id) {
                entry.z = z;
            }
            id
        });
        self.selected = vec![id];
    }

    fn selection_toolbar(&mut self, ui: &mut egui::Ui, palette: &theme::Palette, area: Rect) {
        if self.editing.is_some() || self.selected.is_empty() {
            return;
        }
        let anchor = self
            .selection_bounds()
            .unwrap_or_else(|| Rect::from_center_size(area.center(), Vec2::new(160.0, 40.0)));
        let commands = {
            let mut picker = chrome::FontPicker {
                library: &mut self.fonts,
                installed: &mut self.installed_fonts,
                filter: &mut self.font_filter,
            };
            chrome::selection_toolbar(
                ui,
                palette,
                &self.document,
                &self.selected,
                anchor,
                area,
                &mut picker,
            )
        };
        for command in commands {
            self.apply_command(command);
        }
    }

    fn selection_bounds(&self) -> Option<Rect> {
        let mut bounds = Rect::NOTHING;
        let mut any = false;
        for id in &self.selected {
            let Some(entry) = self.document.entry(*id) else {
                continue;
            };
            if !entry.visible {
                continue;
            }
            let screen = self.view.rect_to_screen(rect_of(entry.geometry));
            bounds = bounds.union(rotated_bounds(screen, entry.geometry.rotation));
            any = true;
        }
        any.then_some(bounds)
    }

    fn apply_command(&mut self, command: chrome::Command) {
        let selected = self.selected.clone();
        if selected.is_empty() {
            return;
        }
        match command {
            chrome::Command::Delete => self.delete_selected(),
            chrome::Command::Duplicate => self.duplicate_selected(),
            chrome::Command::StepTextSize(step) => {
                self.edit(|document| {
                    for id in &selected {
                        if let Some(Body::Text(text)) = document.body_mut(*id) {
                            text.size = chrome::stepped_size(text.size, step);
                        }
                    }
                });
            }
            chrome::Command::SetTextColour(colour) => {
                self.edit(|document| {
                    for id in &selected {
                        if let Some(Body::Text(text)) = document.body_mut(*id) {
                            text.color = colour;
                        }
                    }
                });
            }
            chrome::Command::SetBackground(colour) => {
                self.edit(|document| {
                    for id in &selected {
                        if let Some(Body::Text(text)) = document.body_mut(*id) {
                            text.background = colour;
                        }
                    }
                });
            }
            chrome::Command::SetFont(font) => {
                self.edit(|document| {
                    for id in &selected {
                        if let Some(Body::Text(text)) = document.body_mut(*id) {
                            text.font.clone_from(&font);
                        }
                    }
                });
            }
            chrome::Command::SetImageFit(fit) => {
                self.edit(|document| {
                    for id in &selected {
                        if let Some(Body::Image(image)) = document.body_mut(*id) {
                            image.fit = fit;
                        }
                    }
                });
            }
            chrome::Command::AddTableRow => {
                self.edit(|document| {
                    for id in &selected {
                        if let Some(Body::Table(table)) = document.body_mut(*id) {
                            table.add_row();
                        }
                    }
                });
            }
            chrome::Command::AddTableCol => {
                self.edit(|document| {
                    for id in &selected {
                        if let Some(Body::Table(table)) = document.body_mut(*id) {
                            table.add_col();
                        }
                    }
                });
            }
            chrome::Command::SetShapeKind(kind) => {
                self.edit(|document| {
                    for id in &selected {
                        if let Some(Body::Shape(shape)) = document.body_mut(*id) {
                            shape.shape = kind;
                        }
                    }
                });
            }
            chrome::Command::SetShapeFill(colour) => {
                self.edit(|document| {
                    for id in &selected {
                        if let Some(Body::Shape(shape)) = document.body_mut(*id) {
                            shape.fill = colour;
                        }
                    }
                });
            }
            chrome::Command::SetShapeStroke(colour) => {
                self.edit(|document| {
                    for id in &selected {
                        if let Some(Body::Shape(shape)) = document.body_mut(*id) {
                            shape.stroke = colour;
                        }
                    }
                });
            }
            chrome::Command::SetHighlightColour(colour) => {
                self.edit(|document| {
                    for id in &selected {
                        if let Some(Body::Highlight(highlight)) = document.body_mut(*id) {
                            highlight.color = colour;
                        }
                    }
                });
            }
            chrome::Command::SetHighlightOpacity(opacity) => {
                self.edit(|document| {
                    for id in &selected {
                        if let Some(Body::Highlight(highlight)) = document.body_mut(*id) {
                            highlight.opacity = opacity.clamp(0.0, 1.0);
                        }
                    }
                });
            }
            chrome::Command::Align(alignment) => {
                self.edit(|document| document.align(&selected, alignment));
            }
            chrome::Command::Distribute(axis) => {
                self.edit(|document| document.distribute(&selected, axis));
            }
            chrome::Command::LayerFront => {
                self.edit(|document| document.bring_to_front(&selected));
            }
            chrome::Command::LayerBack => {
                self.edit(|document| document.send_to_back(&selected));
            }
            chrome::Command::Lock(locked) => {
                self.edit(|document| document.set_locked(&selected, locked));
            }
            chrome::Command::Show(visible) => {
                self.edit(|document| document.set_visible(&selected, visible));
            }
        }
    }

    // ---- 绘制 ----

    fn draw_order(&self) -> Vec<usize> {
        let entries = &self.document.board.entries;
        let mut order: Vec<usize> = (0..entries.len()).collect();
        order.sort_by_key(|index| (entries[*index].z, *index));
        order
    }

    fn paint_blocks(&mut self, ui: &egui::Ui, palette: &theme::Palette, clip: Rect) {
        let painter = ui.painter().with_clip_rect(clip);
        let zoom = self.view.zoom;
        for index in self.draw_order() {
            let entry = self.document.board.entries[index].clone();
            if !entry.visible {
                continue;
            }
            let screen = self.view.rect_to_screen(rect_of(entry.geometry));
            if !clip.intersects(screen) {
                continue;
            }
            let editing = self.is_editing_block(entry.id);
            // 编辑框转不了，进编辑时先把这块摆正，写完再转回去。
            let rotation = if editing {
                0.0
            } else {
                entry.geometry.rotation
            };
            let bounds = rotated_bounds(screen, rotation);
            let block_painter = painter.with_clip_rect(clip.intersect(bounds.expand(1.0)));

            match entry.kind {
                BlockKind::Text => self.paint_text(&block_painter, &entry, rotation, zoom, palette),
                BlockKind::Image => {
                    self.paint_image(&block_painter, &entry, rotation, zoom, palette);
                }
                BlockKind::Video | BlockKind::Audio => {
                    self.paint_media(&block_painter, &entry, rotation, zoom, palette);
                }
                BlockKind::Table => {
                    self.paint_table(&block_painter, &entry, rotation, zoom, palette)
                }
                BlockKind::Shape => {
                    self.paint_shape(&block_painter, &entry, rotation, zoom, palette)
                }
                BlockKind::Highlight => self.paint_highlight(&block_painter, &entry, rotation),
            }

            if self.is_selected(entry.id) && !editing {
                dashed_outline(
                    &block_painter,
                    screen,
                    rotation,
                    Stroke::new((theme::EDGE_WIDTH * zoom).max(0.6), palette.text),
                    zoom,
                );
            }
        }
    }

    fn paint_text(
        &self,
        painter: &egui::Painter,
        entry: &Entry,
        rotation: f32,
        zoom: f32,
        palette: &theme::Palette,
    ) {
        let Some(Body::Text(text)) = self.document.body(entry.id) else {
            return;
        };
        let screen = self.view.rect_to_screen(rect_of(entry.geometry));
        if let Some([r, g, b]) = text.background {
            painter.add(rotated_fill(screen, rotation, Color32::from_rgb(r, g, b)));
        }
        if self.is_editing_block(entry.id) {
            return;
        }
        let colour = chrome::text_colour(text, palette);
        let font = FontId::new(
            (text.size.unwrap_or(theme::BLOCK_FONT) * zoom).max(4.0),
            self.family_of(text),
        );
        let galley = painter.layout(text.text.clone(), font, colour, screen.width().max(1.0));
        painter.add(rotated_text(
            screen.min,
            galley,
            colour,
            rotation,
            screen.center(),
        ));
    }

    fn paint_image(
        &mut self,
        painter: &egui::Painter,
        entry: &Entry,
        rotation: f32,
        zoom: f32,
        palette: &theme::Palette,
    ) {
        let Some(Body::Image(image)) = self.document.body(entry.id) else {
            return;
        };
        let image = image.clone();
        let screen = self.view.rect_to_screen(rect_of(entry.geometry));
        let centre = screen.center();
        let Some(texture) = self
            .images
            .texture(painter.ctx(), &image.media, &*self.media)
        else {
            paint_missing_media(painter, screen, rotation, palette, zoom);
            return;
        };
        let source = texture.size;
        if source.x <= 0.0 || source.y <= 0.0 {
            paint_missing_media(painter, screen, rotation, palette, zoom);
            return;
        }

        let (target, uv) = image_frame(image.fit, screen, source);
        if image.fit == ImageFit::Contain {
            painter.add(rotated_fill(screen, rotation, palette.media_fill));
        }
        let corners = rotated_points(target, centre, rotation);
        let uv = [
            Pos2::new(uv.min.x, uv.min.y),
            Pos2::new(uv.max.x, uv.min.y),
            Pos2::new(uv.max.x, uv.max.y),
            Pos2::new(uv.min.x, uv.max.y),
        ];
        let mut mesh = egui::Mesh::with_texture(texture.id);
        for (position, uv) in corners.into_iter().zip(uv) {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: position,
                uv,
                color: Color32::WHITE,
            });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
    }

    fn paint_media(
        &self,
        painter: &egui::Painter,
        entry: &Entry,
        rotation: f32,
        zoom: f32,
        palette: &theme::Palette,
    ) {
        let Some(Body::Media(media)) = self.document.body(entry.id) else {
            return;
        };
        let screen = self.view.rect_to_screen(rect_of(entry.geometry));
        let centre = screen.center();
        painter.add(rotated_fill(screen, rotation, palette.media_fill));
        painter.add(egui::Shape::closed_line(
            rotated_corners(screen, rotation).to_vec(),
            Stroke::new((theme::EDGE_WIDTH * zoom).max(0.5), palette.edge),
        ));

        let small = (theme::SMALL_FONT * zoom).max(4.0);
        let title_font = FontId::proportional((theme::BLOCK_FONT * zoom * 0.85).max(small));
        let kind = match entry.kind {
            BlockKind::Video => strings::KIND_VIDEO,
            BlockKind::Audio => strings::KIND_AUDIO,
            _ => strings::KIND_IMAGE,
        };
        let title = painter.layout_no_wrap(kind.to_owned(), title_font, palette.text);
        let detail_text = if media.name.is_empty() {
            format_bytes(media.bytes)
        } else {
            format!("{} · {}", media.name, format_bytes(media.bytes))
        };
        let detail = painter.layout(
            detail_text,
            FontId::proportional(small),
            palette.muted,
            (screen.width() - theme::TILE_PAD * 2.0).max(1.0),
        );
        let gap = 4.0;
        let total = title.size().y + detail.size().y + gap;
        let top = centre.y - total / 2.0;
        let title_pos = Pos2::new(centre.x - title.size().x / 2.0, top);
        let detail_pos = Pos2::new(centre.x - detail.size().x / 2.0, top + title.size().y + gap);
        painter.add(rotated_text(
            title_pos,
            title,
            palette.text,
            rotation,
            centre,
        ));
        painter.add(rotated_text(
            detail_pos,
            detail,
            palette.muted,
            rotation,
            centre,
        ));
    }

    fn paint_table(
        &self,
        painter: &egui::Painter,
        entry: &Entry,
        rotation: f32,
        zoom: f32,
        palette: &theme::Palette,
    ) {
        let Some(Body::Table(table)) = self.document.body(entry.id) else {
            return;
        };
        let screen = self.view.rect_to_screen(rect_of(entry.geometry));
        let centre = screen.center();
        painter.add(rotated_fill(screen, rotation, palette.block_fill));

        let rows = table.rows.max(1);
        let cols = table.cols.max(1);
        let cell_width = screen.width() / f32::from(cols);
        let cell_height = screen.height() / f32::from(rows);
        let stroke = Stroke::new((theme::EDGE_WIDTH * zoom).max(0.5), palette.edge);

        for column in 0..=cols {
            let x = screen.left() + f32::from(column) * cell_width;
            let points = [Pos2::new(x, screen.top()), Pos2::new(x, screen.bottom())]
                .map(|point| centre + rotate(point - centre, rotation));
            painter.line_segment(points, stroke);
        }
        for row in 0..=rows {
            let y = screen.top() + f32::from(row) * cell_height;
            let points = [Pos2::new(screen.left(), y), Pos2::new(screen.right(), y)]
                .map(|point| centre + rotate(point - centre, rotation));
            painter.line_segment(points, stroke);
        }

        let font = FontId::proportional((theme::SMALL_FONT * zoom).max(4.0));
        let pad = theme::CELL_PAD * zoom;
        let editing_cell = self.editing_cell(entry.id);
        for row in 0..rows {
            for column in 0..cols {
                if editing_cell == Some((row, column)) {
                    continue;
                }
                let text = table.cell(row, column);
                if text.is_empty() {
                    continue;
                }
                let cell = Rect::from_min_size(
                    Pos2::new(
                        screen.left() + f32::from(column) * cell_width + pad,
                        screen.top() + f32::from(row) * cell_height + pad,
                    ),
                    Vec2::new(
                        (cell_width - pad * 2.0).max(1.0),
                        (cell_height - pad * 2.0).max(1.0),
                    ),
                );
                let galley =
                    painter.layout(text.to_owned(), font.clone(), palette.text, cell.width());
                painter.add(rotated_text(
                    cell.min,
                    galley,
                    palette.text,
                    rotation,
                    centre,
                ));
            }
        }
    }

    fn paint_shape(
        &self,
        painter: &egui::Painter,
        entry: &Entry,
        rotation: f32,
        zoom: f32,
        palette: &theme::Palette,
    ) {
        let Some(Body::Shape(shape)) = self.document.body(entry.id) else {
            return;
        };
        let screen = self.view.rect_to_screen(rect_of(entry.geometry));
        let centre = screen.center();
        let fill = shape.fill.map(|[r, g, b]| Color32::from_rgb(r, g, b));
        let stroke = shape.stroke.map(|[r, g, b]| {
            Stroke::new(
                (shape.stroke_width * zoom).max(0.5),
                Color32::from_rgb(r, g, b),
            )
        });

        match shape.shape {
            ShapeKind::Rect => {
                if let Some(fill) = fill {
                    painter.add(rotated_fill(screen, rotation, fill));
                }
                if let Some(stroke) = stroke {
                    painter.add(egui::Shape::closed_line(
                        rotated_corners(screen, rotation).to_vec(),
                        stroke,
                    ));
                }
            }
            ShapeKind::Ellipse => {
                let mut ellipse = egui::epaint::EllipseShape::filled(
                    centre,
                    screen.size() / 2.0,
                    fill.unwrap_or(Color32::TRANSPARENT),
                );
                ellipse.stroke = stroke.unwrap_or(Stroke::NONE);
                ellipse.angle = rotation;
                painter.add(egui::Shape::Ellipse(ellipse));
            }
            ShapeKind::Line => {
                let stroke =
                    stroke.unwrap_or_else(|| Stroke::new(1.0, fill.unwrap_or(palette.text)));
                let half = Vec2::new(screen.width() / 2.0, 0.0);
                painter.line_segment(
                    [
                        centre - rotate(half, rotation),
                        centre + rotate(half, rotation),
                    ],
                    stroke,
                );
            }
        }
    }

    fn paint_highlight(&self, painter: &egui::Painter, entry: &Entry, rotation: f32) {
        let Some(Body::Highlight(HighlightBlock { color, opacity })) = self.document.body(entry.id)
        else {
            return;
        };
        let screen = self.view.rect_to_screen(rect_of(entry.geometry));
        let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        let [r, g, b] = *color;
        painter.add(rotated_fill(
            screen,
            rotation,
            Color32::from_rgba_unmultiplied(r, g, b, alpha),
        ));
    }

    fn paint_marquee(&self, ui: &egui::Ui, palette: &theme::Palette, clip: Rect) {
        let Some(marquee) = &self.marquee else {
            return;
        };
        let start = self.view.to_screen(marquee.start);
        let current = self.view.to_screen(marquee.current);
        let rect = Rect::from_two_pos(start, current);
        let [r, g, b, _] = palette.text.to_srgba_unmultiplied();
        ui.painter().with_clip_rect(clip).rect(
            rect,
            CornerRadius::ZERO,
            Color32::from_rgba_unmultiplied(r, g, b, theme::MARQUEE_ALPHA),
            Stroke::new(1.0, palette.muted),
            StrokeKind::Inside,
        );
    }

    fn paint_hud(&mut self, ui: &egui::Ui, palette: &theme::Palette, rect: Rect) {
        let dt = ui.input(|input| input.stable_dt).max(1.0e-4);
        let instant = 1.0 / dt;
        self.fps = if self.fps == 0.0 {
            instant
        } else {
            self.fps * 0.9 + instant * 0.1
        };

        let painter = ui.painter();
        if self.document.board.entries.is_empty() {
            let centre = rect.center();
            painter.text(
                centre - Vec2::new(0.0, 10.0),
                Align2::CENTER_CENTER,
                strings::EMPTY_BOARD,
                FontId::proportional(theme::BLOCK_FONT),
                palette.muted,
            );
            painter.text(
                centre + Vec2::new(0.0, 12.0),
                Align2::CENTER_CENTER,
                strings::EMPTY_BOARD_HINT,
                FontId::proportional(theme::SMALL_FONT),
                palette.muted,
            );
        }

        let font = FontId::proportional(theme::SMALL_FONT);
        let hovered_files = ui.input(|input| !input.raw.hovered_files.is_empty());
        let hint = if hovered_files {
            strings::INSERT_MEDIA_HINT
        } else {
            strings::BOARD_HINT
        };
        let bottom = rect.left_bottom() + Vec2::new(theme::HINT_INSET, -theme::HINT_INSET);
        paint_chip(painter, bottom, Align2::LEFT_BOTTOM, hint, &font, palette);

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

    fn edit_text(&mut self, ui: &mut egui::Ui, palette: &theme::Palette) {
        let Some(Editing::Text(id)) = &self.editing else {
            return;
        };
        let id = *id;
        let Some(entry) = self.document.entry(id).cloned() else {
            self.editing = None;
            return;
        };
        let zoom = self.view.zoom;
        let screen = self.view.rect_to_screen(rect_of(entry.geometry));
        let Some(Body::Text(text)) = self.document.body(id) else {
            self.editing = None;
            return;
        };
        let size = text.size.unwrap_or(theme::BLOCK_FONT);
        let family = self.family_of(text);
        let colour = chrome::text_colour(text, palette);
        let background = text
            .background
            .map_or(Color32::TRANSPARENT, |[r, g, b]| Color32::from_rgb(r, g, b));
        let font = FontId::new((size * zoom).max(4.0), family);
        let editor_id = ui.scope_id().with(("text-editor", id));

        let response = {
            let Some(Body::Text(text)) = self.document.body_mut(id) else {
                return;
            };
            let editor = egui::TextEdit::multiline(&mut text.text)
                .id(editor_id)
                .font(font.clone())
                .text_color(colour)
                .background_color(background)
                .margin(egui::Margin::ZERO)
                .frame(egui::Frame::NONE)
                .desired_width(screen.width())
                .desired_rows(1);
            ui.put(screen, editor)
        };

        // 焦点要等编辑框建好之后再要：它建起来时会因为「这次点击落在框外」而放弃焦点，
        // 先要就被立刻清掉。放到后面要，焦点留到下一帧，用户双击完就能直接打字。
        if self.request_focus {
            ui.memory_mut(|memory| memory.request_focus(editor_id));
            self.request_focus = false;
        }

        // 字变多就长高：这一步算在正在编辑的这一块里，结束时随撤销一起提交。
        let measured = {
            let Some(Body::Text(text)) = self.document.body(id) else {
                return;
            };
            let galley =
                ui.painter()
                    .layout(text.text.clone(), font, colour, screen.width().max(1.0));
            galley.size().y.max(theme::MIN_BLOCK_HEIGHT)
        };
        if let Some(entry) = self.document.entry_mut(id)
            && measured > entry.geometry.height
        {
            entry.geometry.height = measured;
        }

        if response.lost_focus() {
            self.commit_editing();
        }
    }

    fn edit_table(&mut self, ui: &mut egui::Ui, palette: &theme::Palette) {
        let Some(Editing::Table { id, cell, .. }) = &self.editing else {
            return;
        };
        let (id, cell) = (*id, *cell);
        let Some(entry) = self.document.entry(id).cloned() else {
            self.editing = None;
            return;
        };
        let Some(Body::Table(table)) = self.document.body(id) else {
            self.editing = None;
            return;
        };
        let rows = table.rows.max(1);
        let cols = table.cols.max(1);
        let zoom = self.view.zoom;
        let screen = self.view.rect_to_screen(rect_of(entry.geometry));
        let Some((row, column)) = cell else {
            return;
        };
        let cell_rect = table_cell_rect(screen, rows, cols, row, column);
        let font = FontId::proportional((theme::SMALL_FONT * zoom).max(4.0));
        let editor_id = ui.scope_id().with(("table-editor", id, row, column));

        let response = {
            let Some(Editing::Table { draft, .. }) = &mut self.editing else {
                return;
            };
            let editor = egui::TextEdit::singleline(draft)
                .id(editor_id)
                .font(font)
                .text_color(palette.text)
                .margin(egui::Margin::ZERO)
                .frame(egui::Frame::NONE)
                .desired_width(cell_rect.width());
            ui.put(cell_rect.shrink2(Vec2::new(2.0, 2.0)), editor)
        };

        if self.request_focus {
            ui.memory_mut(|memory| memory.request_focus(editor_id));
            self.request_focus = false;
        }

        let pointer = ui.input(|input| input.pointer.interact_pos());
        let clicked = ui.input(|input| input.pointer.primary_clicked());
        if clicked {
            let inside = pointer.is_some_and(|pointer| screen.contains(pointer));
            if inside {
                let next = pointer.and_then(|pointer| self.table_cell_at(id, pointer));
                self.commit_table_cell();
                if let Some((row, column)) = next {
                    let draft = self.table_cell_text(id, row, column);
                    if let Some(Editing::Table {
                        cell,
                        draft: buffer,
                        ..
                    }) = &mut self.editing
                    {
                        *cell = Some((row, column));
                        *buffer = draft;
                    }
                    self.request_focus = true;
                }
            } else {
                self.commit_editing();
            }
            return;
        }

        if response.lost_focus() {
            self.commit_editing();
        }
    }

    // ---- 字体与文本样式 ----

    fn ensure_fonts(&mut self, context: &egui::Context) {
        let installed = &self.installed_fonts;
        let mut missing: Vec<String> = self
            .document
            .bodies
            .values()
            .filter_map(|body| match body {
                Body::Text(text) => text.font.clone(),
                _ => None,
            })
            .filter(|name| !installed.contains(name))
            .collect();
        missing.sort();
        missing.dedup();
        if missing.is_empty() {
            return;
        }

        if self.fonts.is_none() {
            self.fonts = Some(crate::fonts::FontLibrary::load());
        }
        let Some(library) = &self.fonts else {
            return;
        };
        for name in missing {
            if let Some(index) = library.index_of(&name) {
                library.install(context, index);
            }
            // 装上了、或者这台机器没这个字体，都记下来，别每帧重试。
            self.installed_fonts.insert(name);
        }
        context.request_repaint();
    }

    fn family_of(&self, text: &TextBlock) -> egui::FontFamily {
        match text.font.as_deref() {
            Some(name) if self.installed_fonts.contains(name) => {
                egui::FontFamily::Name(name.into())
            }
            _ => egui::FontFamily::Proportional,
        }
    }

    // ---- 小工具 ----

    fn is_selected(&self, id: u64) -> bool {
        self.selected.contains(&id)
    }

    fn select_only(&mut self, id: u64) {
        self.selected = vec![id];
    }

    fn toggle_selection(&mut self, id: u64) {
        if let Some(index) = self.selected.iter().position(|selected| *selected == id) {
            self.selected.remove(index);
        } else {
            self.selected.push(id);
        }
    }

    fn editing_block(&self) -> Option<u64> {
        match &self.editing {
            Some(Editing::Text(id)) => Some(*id),
            Some(Editing::Table { id, .. }) => Some(*id),
            None => None,
        }
    }

    fn is_editing_block(&self, id: u64) -> bool {
        self.editing_block() == Some(id)
    }

    fn editing_cell(&self, id: u64) -> Option<(u16, u16)> {
        match &self.editing {
            Some(Editing::Table {
                id: current, cell, ..
            }) if *current == id => *cell,
            _ => None,
        }
    }
}

/// 状态文案是不是一条错误。错误沿用 strings 里失败文案的前缀，不需要另行标记。
fn looks_like_error(text: &str) -> bool {
    let probes = [
        strings::save_failed(""),
        strings::open_failed(""),
        strings::workspace_failed("", ""),
        strings::export_failed(""),
        strings::CANNOT_SAVE.to_owned(),
        strings::UNSUPPORTED_FILE.to_owned(),
        strings::MEDIA_READ_FAILED.to_owned(),
    ];
    probes.iter().any(|probe| {
        let head = probe
            .split(|character: char| !character.is_alphabetic())
            .next()
            .unwrap_or_default();
        !head.is_empty() && text.starts_with(head)
    })
}

/// 图片块在框里的排法：返回图片要画的矩形和取用的纹理区域。
fn image_frame(fit: ImageFit, screen: Rect, source: Vec2) -> (Rect, Rect) {
    let full = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
    match fit {
        ImageFit::Fill => (screen, full),
        ImageFit::Contain => {
            let scale = (screen.width() / source.x).min(screen.height() / source.y);
            (
                Rect::from_center_size(screen.center(), source * scale),
                full,
            )
        }
        ImageFit::Cover => {
            let screen_aspect = screen.width() / screen.height();
            let source_aspect = source.x / source.y;
            let uv = if source_aspect > screen_aspect {
                // 图更宽：裁掉左右两边。
                let keep = screen_aspect / source_aspect;
                let margin = (1.0 - keep) / 2.0;
                Rect::from_min_max(Pos2::new(margin, 0.0), Pos2::new(1.0 - margin, 1.0))
            } else {
                // 图更高：裁掉上下两边。
                let keep = source_aspect / screen_aspect;
                let margin = (1.0 - keep) / 2.0;
                Rect::from_min_max(Pos2::new(0.0, margin), Pos2::new(1.0, 1.0 - margin))
            };
            (screen, uv)
        }
    }
}

fn media_size(kind: BlockKind) -> Vec2 {
    match kind {
        BlockKind::Video => Vec2::new(320.0, 180.0),
        BlockKind::Audio => Vec2::new(280.0, 96.0),
        _ => Vec2::new(320.0, 240.0),
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

fn paint_missing_media(
    painter: &egui::Painter,
    screen: Rect,
    rotation: f32,
    palette: &theme::Palette,
    zoom: f32,
) {
    painter.add(rotated_fill(screen, rotation, palette.media_fill));
    painter.add(egui::Shape::closed_line(
        rotated_corners(screen, rotation).to_vec(),
        Stroke::new((theme::EDGE_WIDTH * zoom).max(0.5), palette.edge),
    ));
    let font = FontId::proportional((theme::SMALL_FONT * zoom).max(6.0));
    let galley = painter.layout(
        strings::MEDIA_MISSING.to_owned(),
        font,
        palette.muted,
        (screen.width() - theme::TILE_PAD * 2.0).max(1.0),
    );
    let position = screen.center() - galley.size() / 2.0;
    painter.add(rotated_text(
        position,
        galley,
        palette.muted,
        rotation,
        screen.center(),
    ));
}

fn table_cell_rect(screen: Rect, rows: u16, cols: u16, row: u16, column: u16) -> Rect {
    let cell_width = screen.width() / f32::from(cols.max(1));
    let cell_height = screen.height() / f32::from(rows.max(1));
    Rect::from_min_size(
        Pos2::new(
            screen.left() + f32::from(column) * cell_width,
            screen.top() + f32::from(row) * cell_height,
        ),
        Vec2::new(cell_width, cell_height),
    )
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
    let corners = rotated_corners(rect, rotation);
    for index in 0..4 {
        painter.extend(egui::Shape::dashed_line(
            &[corners[index], corners[(index + 1) % 4]],
            stroke,
            4.0 * zoom,
            3.0 * zoom,
        ));
    }
}

/// 块的四个角，按旋转之后的屏幕位置给。
fn rotated_corners(rect: Rect, rotation: f32) -> [Pos2; 4] {
    rotated_points(rect, rect.center(), rotation)
}

fn rotated_points(rect: Rect, pivot: Pos2, rotation: f32) -> [Pos2; 4] {
    [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
    .map(|corner| pivot + rotate(corner - pivot, rotation))
}

fn rotated_fill(rect: Rect, rotation: f32, colour: Color32) -> egui::Shape {
    egui::Shape::convex_polygon(
        rotated_corners(rect, rotation).to_vec(),
        colour,
        Stroke::NONE,
    )
}

/// 一段排版好的文字，绕 pivot（一般是块中心）旋转，而不是绕自己的左上角。
fn rotated_text(
    position: Pos2,
    galley: std::sync::Arc<egui::Galley>,
    colour: Color32,
    rotation: f32,
    pivot: Pos2,
) -> egui::Shape {
    let position = pivot + rotate(position - pivot, rotation);
    egui::Shape::Text(egui::epaint::TextShape::new(position, galley, colour).with_angle(rotation))
}

/// 旋转之后的包围盒，用来摆浮动的操作条。
fn rotated_bounds(rect: Rect, rotation: f32) -> Rect {
    Rect::from_points(&rotated_corners(rect, rotation))
}

fn rect_of(geometry: Geometry) -> Rect {
    Rect::from_min_size(
        Pos2::new(geometry.x, geometry.y),
        Vec2::new(geometry.width, geometry.height),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank() -> BoardApp {
        BoardApp::new(Document::new("测试画板"), Box::new(()))
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

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn click_once(ctx: &egui::Context, app: &mut BoardApp, pointer: Pos2, time: f64) {
        frame(
            ctx,
            app,
            time,
            vec![egui::Event::PointerMoved(pointer), click(pointer, true)],
        );
        frame(ctx, app, time + 0.05, vec![click(pointer, false)]);
    }

    /// 双击一块或空白。返回时多半处于编辑状态。
    fn double_click(ctx: &egui::Context, app: &mut BoardApp, pointer: Pos2, time: f64) {
        frame(
            ctx,
            app,
            time,
            vec![egui::Event::PointerMoved(pointer), click(pointer, true)],
        );
        frame(ctx, app, time + 0.05, vec![click(pointer, false)]);
        frame(ctx, app, time + 0.10, vec![click(pointer, true)]);
        frame(ctx, app, time + 0.15, vec![click(pointer, false)]);
    }

    fn click_elsewhere(ctx: &egui::Context, app: &mut BoardApp, away: Pos2) {
        frame(
            ctx,
            app,
            0.30,
            vec![egui::Event::PointerMoved(away), click(away, true)],
        );
        frame(ctx, app, 0.35, vec![click(away, false)]);
    }

    fn drag(ctx: &egui::Context, app: &mut BoardApp, from: Pos2, to: Pos2, time: f64) {
        frame(
            ctx,
            app,
            time,
            vec![egui::Event::PointerMoved(from), click(from, true)],
        );
        frame(ctx, app, time + 0.05, vec![egui::Event::PointerMoved(to)]);
        frame(ctx, app, time + 0.10, vec![click(to, false)]);
    }

    #[test]
    fn marquee_drag_selects_every_block_it_touches() {
        let ctx = egui::Context::default();
        let mut document = Document::new("测试画板");
        let first = document.add_text(Geometry::new(0.0, 0.0, 50.0, 50.0));
        let second = document.add_text(Geometry::new(100.0, 0.0, 50.0, 50.0));
        let mut app = BoardApp::new(document, Box::new(()));
        frame(&ctx, &mut app, 0.0, Vec::new());

        let from = app.view.to_screen(Pos2::new(-40.0, -40.0));
        let to = app.view.to_screen(Pos2::new(180.0, 80.0));
        drag(&ctx, &mut app, from, to, 0.10);

        assert!(app.marquee.is_none());
        assert_eq!(app.selected.len(), 2);
        assert!(app.selected.contains(&first));
        assert!(app.selected.contains(&second));
    }

    #[test]
    fn dragging_moves_unlocked_blocks_only() {
        let ctx = egui::Context::default();
        let mut document = Document::new("测试画板");
        let locked = document.add_text(Geometry::new(0.0, 0.0, 100.0, 50.0));
        let free = document.add_text(Geometry::new(0.0, 100.0, 100.0, 50.0));
        document.set_locked(&[locked], true);
        let mut app = BoardApp::new(document, Box::new(()));
        frame(&ctx, &mut app, 0.0, Vec::new());

        let locked_centre = app.view.to_screen(Pos2::new(50.0, 25.0));
        drag(
            &ctx,
            &mut app,
            locked_centre,
            locked_centre + Vec2::new(80.0, 0.0),
            0.10,
        );
        let still = app.document().entry(locked).expect("块还在").geometry.x;
        assert!(still.abs() < 0.001, "锁定的块动了：{still}");

        let free_centre = app.view.to_screen(Pos2::new(50.0, 125.0));
        drag(
            &ctx,
            &mut app,
            free_centre,
            free_centre + Vec2::new(80.0, 0.0),
            0.40,
        );
        let moved = app.document().entry(free).expect("块还在").geometry.x;
        assert!((moved - 80.0).abs() < 1.0, "位移是 {moved}");
    }

    #[test]
    fn every_block_kind_paints_without_panicking() {
        let ctx = egui::Context::default();
        let mut document = Document::new("测试画板");
        let mut ids = Vec::new();
        for (index, kind) in BlockKind::ALL.iter().enumerate() {
            let x = (index % 4) as f32 * 180.0;
            let y = (index / 4) as f32 * 140.0;
            let id = document.add_block(*kind, Geometry::new(x, y, 150.0, 100.0));
            ids.push(id);
            match document.body_mut(id) {
                Some(Body::Text(text)) => text.text = "abc".to_owned(),
                Some(Body::Table(table)) => {
                    table.set_cell(0, 0, "x");
                    table.set_cell(1, 1, "y");
                }
                Some(Body::Shape(shape)) => shape.stroke = Some([0, 0, 0]),
                _ => {}
            }
        }
        document
            .entry_mut(ids[0])
            .expect("块还在")
            .geometry
            .rotation = 0.3;
        let mut app = BoardApp::new(document, Box::new(()));

        app.selected = ids.clone();
        frame(&ctx, &mut app, 0.0, Vec::new());
        for (index, id) in ids.into_iter().enumerate() {
            app.selected = vec![id];
            frame(&ctx, &mut app, 0.10 + index as f64 * 0.10, Vec::new());
        }
    }

    #[test]
    fn multi_selection_toolbar_paints() {
        let ctx = egui::Context::default();
        let mut document = Document::new("测试画板");
        for index in 0..3 {
            document.add_text(Geometry::new(index as f32 * 200.0, 0.0, 100.0, 50.0));
        }
        let mut app = BoardApp::new(document, Box::new(()));
        let ids: Vec<u64> = app
            .document()
            .board
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect();
        app.selected = ids.clone();
        frame(&ctx, &mut app, 0.0, Vec::new());

        // 选区里没有可见块时，操作条仍要出现，否则隐藏的块没法再显示出来。
        app.edit(|document| document.set_visible(&ids, false));
        frame(&ctx, &mut app, 0.1, Vec::new());
    }

    #[test]
    fn world_and_screen_roundtrip() {
        let app = blank();
        let world = Pos2::new(123.0, 45.0);
        let back = app.view.to_world(app.view.to_screen(world));
        assert!((back - world).length() < 0.001);
    }

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let mut app = blank();
        let pointer = Pos2::new(400.0, 300.0);
        let before = app.view.to_world(pointer);
        app.zoom_at(pointer, 1.8);
        let after = app.view.to_world(pointer);
        assert!((after - before).length() < 0.001);
        assert!((app.view.zoom - 1.8).abs() < 0.001);
    }

    #[test]
    fn zoom_stays_in_range() {
        let mut app = blank();
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
    fn double_click_makes_a_block_that_takes_typing() {
        let ctx = egui::Context::default();
        let mut app = blank();
        let pointer = Pos2::new(400.0, 300.0);

        double_click(&ctx, &mut app, pointer, 0.0);
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
        let mut app = blank();
        let pointer = Pos2::new(400.0, 300.0);

        double_click(&ctx, &mut app, pointer, 0.0);
        // 还没确认：这块既没有虚线框，也不算被选中。
        assert!(app.selected.is_empty());

        frame(
            &ctx,
            &mut app,
            0.20,
            vec![egui::Event::Text("你好".to_owned())],
        );
        click_elsewhere(&ctx, &mut app, Pos2::new(1000.0, 700.0));

        assert!(app.editing.is_none());
        assert_eq!(app.selected, vec![1]);
        assert_eq!(app.document().text_of(1), "你好");
        assert_eq!(app.document().board.entries.len(), 1);
    }

    #[test]
    fn an_empty_block_disappears_when_committed() {
        let ctx = egui::Context::default();
        let mut app = blank();

        double_click(&ctx, &mut app, Pos2::new(400.0, 300.0), 0.0);
        assert_eq!(app.document().board.entries.len(), 1);
        click_elsewhere(&ctx, &mut app, Pos2::new(1000.0, 700.0));

        assert!(app.document().board.entries.is_empty());
        assert!(app.selected.is_empty());
    }

    #[test]
    fn typing_and_committing_is_one_undo_step() {
        let ctx = egui::Context::default();
        let mut app = blank();
        double_click(&ctx, &mut app, Pos2::new(400.0, 300.0), 0.0);
        frame(
            &ctx,
            &mut app,
            0.20,
            vec![egui::Event::Text("abc".to_owned())],
        );
        click_elsewhere(&ctx, &mut app, Pos2::new(1000.0, 700.0));
        assert!(app.history.can_undo());
        app.undo();
        assert!(app.document().board.entries.is_empty());
    }

    #[test]
    fn shift_click_toggles_selection() {
        let ctx = egui::Context::default();
        let mut document = Document::new("测试画板");
        let first = document.add_text(Geometry::new(0.0, 0.0, 100.0, 50.0));
        let second = document.add_text(Geometry::new(220.0, 0.0, 100.0, 50.0));
        let mut app = BoardApp::new(document, Box::new(()));
        frame(&ctx, &mut app, 0.0, Vec::new());

        let first_point = app.view.to_screen(Pos2::new(50.0, 25.0));
        let second_point = app.view.to_screen(Pos2::new(270.0, 25.0));

        click_once(&ctx, &mut app, first_point, 0.10);
        assert_eq!(app.selected, vec![first]);

        frame(
            &ctx,
            &mut app,
            0.20,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::SHIFT)],
        );
        click_once(&ctx, &mut app, second_point, 0.25);
        assert_eq!(app.selected, vec![first, second]);

        click_once(&ctx, &mut app, second_point, 0.40);
        assert_eq!(app.selected, vec![first]);
        frame(
            &ctx,
            &mut app,
            0.60,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)],
        );
    }

    #[test]
    fn hidden_blocks_do_not_take_clicks() {
        let ctx = egui::Context::default();
        let mut document = Document::new("测试画板");
        let id = document.add_text(Geometry::new(0.0, 0.0, 100.0, 50.0));
        document.entry_mut(id).expect("刚加的块").visible = false;
        let mut app = BoardApp::new(document, Box::new(()));
        frame(&ctx, &mut app, 0.0, Vec::new());

        let point = app.view.to_screen(Pos2::new(50.0, 25.0));
        click_once(&ctx, &mut app, point, 0.10);
        assert!(app.selected.is_empty());
    }

    #[test]
    fn delete_and_undo_roundtrip() {
        let ctx = egui::Context::default();
        let mut document = Document::new("测试画板");
        let id = document.add_text(Geometry::new(0.0, 0.0, 100.0, 50.0));
        document.text_mut(id).text = "一行字".to_owned();
        let mut app = BoardApp::new(document, Box::new(()));
        frame(&ctx, &mut app, 0.0, Vec::new());

        app.select_first();
        frame(
            &ctx,
            &mut app,
            0.10,
            vec![key(egui::Key::Delete, egui::Modifiers::NONE)],
        );
        assert!(app.document().board.entries.is_empty());

        frame(
            &ctx,
            &mut app,
            0.20,
            vec![
                egui::Event::ModifiersChanged(egui::Modifiers::COMMAND),
                key(egui::Key::Z, egui::Modifiers::COMMAND),
            ],
        );
        assert_eq!(app.document().board.entries.len(), 1);

        frame(
            &ctx,
            &mut app,
            0.30,
            vec![key(egui::Key::Y, egui::Modifiers::COMMAND)],
        );
        assert!(app.document().board.entries.is_empty());
        frame(
            &ctx,
            &mut app,
            0.40,
            vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)],
        );
    }

    #[test]
    fn error_status_beats_unsaved_and_normal_status_expires() {
        let mut app = blank();
        app.edit(|document| {
            document.add_text(Geometry::new(0.0, 0.0, 10.0, 10.0));
        });
        assert!(app.is_dirty());

        app.set_status("普通提示");
        assert_eq!(app.status_text(0.0).0, strings::UNSAVED);

        app.set_error(strings::save_failed("磁盘满了"));
        let (text, error) = app.status_text(0.0);
        assert!(error);
        assert_eq!(text, strings::save_failed("磁盘满了"));

        app.mark_saved();
        app.set_status(strings::SAVED);
        app.status.as_mut().expect("刚设过状态").born = Some(10.0);
        assert_eq!(app.status_text(10.5).0, strings::SAVED);
        assert_eq!(app.status_text(14.0).0, "");
    }

    #[test]
    fn replace_document_clears_history_and_dirty() {
        let mut app = blank();
        app.edit(|document| {
            document.add_text(Geometry::new(0.0, 0.0, 10.0, 10.0));
        });
        assert!(app.history.can_undo());
        app.replace_document(Document::new("另一张画板"));
        assert!(!app.history.can_undo());
        assert!(!app.is_dirty());
        assert!(app.selected.is_empty());
        assert!(!app.take_image_export_request());
    }

    #[test]
    fn canvas_rect_excludes_the_top_bar() {
        let ctx = egui::Context::default();
        let mut app = blank();
        frame(&ctx, &mut app, 0.0, Vec::new());

        let canvas = app.canvas_rect();
        assert!((canvas.left()).abs() < 0.001);
        assert!((canvas.top() - chrome::TOP_BAR_HEIGHT).abs() < 0.001);
        assert!((canvas.width() - 1280.0).abs() < 0.001);
        assert!((canvas.height() - (760.0 - chrome::TOP_BAR_HEIGHT)).abs() < 0.001);

        assert!(!app.take_image_export_request());
        app.image_export_request = true;
        assert!(app.take_image_export_request());
        assert!(!app.take_image_export_request());
    }

    #[test]
    fn changed_bodies_tracks_only_body_edits() {
        let mut app = blank();
        app.mark_saved();

        let first = app.edit(|document| document.add_text(Geometry::new(0.0, 0.0, 100.0, 50.0)));
        assert_eq!(app.changed_bodies(), vec![first]);
        app.mark_saved();
        assert!(app.changed_bodies().is_empty());

        app.edit(|document| {
            if let Some(Body::Text(text)) = document.body_mut(first) {
                text.text = "改动".to_owned();
            }
        });
        assert_eq!(app.changed_bodies(), vec![first]);
        app.mark_saved();

        // 只挪位置、改层级不算正文改动。
        app.edit(|document| {
            if let Some(entry) = document.entry_mut(first) {
                entry.geometry.x += 40.0;
                entry.z += 1;
            }
        });
        assert!(app.changed_bodies().is_empty());
        app.mark_saved();

        let second = app.edit(|document| document.add_text(Geometry::new(200.0, 0.0, 100.0, 50.0)));
        assert_eq!(app.changed_bodies(), vec![second]);
        app.mark_saved();
        assert!(app.changed_bodies().is_empty());

        // 删除由保存后的孤儿回收处理，不在增量保存的正文集合里。
        app.edit(|document| {
            document.remove_block(second);
        });
        assert!(app.changed_bodies().is_empty());
    }

    #[test]
    fn nudge_first_text_marks_dirty_and_can_be_undone() {
        let mut document = Document::new("测试画板");
        let id = document.add_text(Geometry::new(0.0, 0.0, 100.0, 50.0));
        document.text_mut(id).text = "abc".to_owned();
        let mut app = BoardApp::new(document, Box::new(()));
        assert!(!app.is_dirty());

        app.nudge_first_text();
        assert!(app.is_dirty());
        assert_eq!(app.document().text_of(id), "abcx");
        assert_eq!(app.changed_bodies(), vec![id]);

        app.undo();
        assert_eq!(app.document().text_of(id), "abc");
        assert!(!app.is_dirty());
        assert!(app.changed_bodies().is_empty());

        // 没有文本块时什么都不做。
        let mut empty = blank();
        empty.nudge_first_text();
        assert!(!empty.is_dirty());
        assert!(empty.changed_bodies().is_empty());
    }

    #[test]
    fn inserting_media_creates_the_matching_body() {
        let mut app = blank();
        app.insert_media(BlockKind::Image, "hash".to_owned(), "a.png".to_owned(), 0);
        let entries = &app.document().board.entries;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, BlockKind::Image);
        assert!(matches!(
            app.document().body(entries[0].id),
            Some(Body::Image(image)) if image.media == "hash" && image.fit == ImageFit::Cover
        ));

        app.insert_media(
            BlockKind::Audio,
            "sound".to_owned(),
            "a.mp3".to_owned(),
            2048,
        );
        let entries = &app.document().board.entries;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].kind, BlockKind::Audio);
        assert!(matches!(
            app.document().body(entries[1].id),
            Some(Body::Media(media)) if media.media == "sound" && media.bytes == 2048
        ));
    }

    #[test]
    fn double_clicking_a_video_asks_for_its_media() {
        let ctx = egui::Context::default();
        let mut document = Document::new("测试画板");
        let id = document.add_block(BlockKind::Video, Geometry::new(0.0, 0.0, 200.0, 100.0));
        document.bodies.insert(
            id,
            Body::Media(MediaBlock {
                media: "clip".to_owned(),
                name: "a.mp4".to_owned(),
                bytes: 10,
            }),
        );
        let mut app = BoardApp::new(document, Box::new(()));
        frame(&ctx, &mut app, 0.0, Vec::new());

        let point = app.view.to_screen(Pos2::new(100.0, 50.0));
        double_click(&ctx, &mut app, point, 0.10);
        assert_eq!(app.take_media_request().as_deref(), Some("clip"));
        assert!(app.take_media_request().is_none());
    }

    #[test]
    fn table_cells_commit_through_history() {
        let mut app = blank();
        let id = app.edit(|document| {
            document.add_block(BlockKind::Table, Geometry::new(0.0, 0.0, 300.0, 150.0))
        });
        app.editing = Some(Editing::Table {
            id,
            cell: Some((0, 0)),
            draft: "甲".to_owned(),
        });
        app.commit_table_cell();
        match app.document().body(id) {
            Some(Body::Table(table)) => assert_eq!(table.cell(0, 0), "甲"),
            _ => panic!("表格块应该还在"),
        }
        app.editing = None;
        app.undo();
        match app.document().body(id) {
            Some(Body::Table(table)) => assert_eq!(table.cell(0, 0), ""),
            _ => panic!("表格块应该还在"),
        }
    }

    #[test]
    fn a_drag_is_one_undo_step() {
        let mut app = blank();
        let id = app.edit(|document| document.add_text(Geometry::new(0.0, 0.0, 100.0, 50.0)));
        let before = app.document.clone();
        for _ in 0..3 {
            if let Some(entry) = app.document.entry_mut(id) {
                entry.geometry.x += 10.0;
            }
        }
        app.commit_change(before);
        assert_eq!(app.document().entry(id).expect("块还在").geometry.x, 30.0);
        app.undo();
        assert_eq!(app.document().entry(id).expect("块还在").geometry.x, 0.0);
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
        let mut app = BoardApp::new(document, Box::new(()));

        app.select_first();
        assert_eq!(app.selected, vec![id]);
        frame(&ctx, &mut app, 0.0, Vec::new());
        assert_eq!(app.selected, vec![id]);
    }

    #[test]
    fn geometry_survives_a_translation() {
        let geometry = Geometry::new(10.0, 20.0, 30.0, 40.0);
        let moved = Geometry::new(geometry.x + 5.0, geometry.y - 5.0, 30.0, 40.0);
        assert_eq!(moved, Geometry::new(15.0, 15.0, 30.0, 40.0));
    }

    #[test]
    fn thumbnail_view_fits_content_then_restores() {
        let mut app = blank();
        app.canvas = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        app.document.add_text(Geometry::new(0.0, 0.0, 100.0, 50.0));
        app.document
            .add_text(Geometry::new(2000.0, 1500.0, 100.0, 50.0));
        let before = app.view;

        assert!(app.begin_thumbnail_view());
        assert!(app.view.zoom < before.zoom);
        let bounds = app.content_bounds().expect("两块内容");
        assert!(app.canvas.contains_rect(app.view.rect_to_screen(bounds)));

        app.end_thumbnail_view();
        assert!((app.view.zoom - before.zoom).abs() < f32::EPSILON);
        assert!((app.view.origin - before.origin).length() < 0.001);
    }

    #[test]
    fn thumbnail_view_does_nothing_on_an_empty_board() {
        let mut app = blank();
        let before = app.view;
        assert!(!app.begin_thumbnail_view());
        assert!((app.view.zoom - before.zoom).abs() < f32::EPSILON);
    }
}
