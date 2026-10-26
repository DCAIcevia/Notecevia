//! 画布之外的界面：顶栏与选中块的操作条。
//!
//! 这里的东西守同一条规矩：没有内容的时候不占地方。顶栏只放返回、新建、画板名与
//! 保存状态；格式操作只在选中块时浮在选区上方，取消选中就消失。

use std::collections::HashSet;

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use notecevia_core::board::{Document, Entry, TextBlock};
use notecevia_core::{Alignment, Axis, BlockKind, Body, ImageFit, ShapeKind};

use crate::{fonts::FontLibrary, strings, theme};

/// 顶栏高度。画布从这条线以下开始。
pub const TOP_BAR_HEIGHT: f32 = 40.0;

const TOOLBAR_ROW_HEIGHT: f32 = 28.0;
const TOOLBAR_ROW_GAP: f32 = 6.0;
const TOOLBAR_ITEM_GAP: f32 = 4.0;
const TOOLBAR_GAP: f32 = 8.0;
const BAR_PAD: f32 = 8.0;
/// 操作条一行最宽多少；再宽就折行，不挡内容。
const MAX_ROW_WIDTH: f32 = 560.0;
const RADIUS: u8 = 6;
const RADIUS_SMALL: u8 = 4;

/// 顶栏上按出来的动作。
pub struct TopBarOutput {
    pub home: bool,
    pub new_kind: Option<BlockKind>,
}

/// 顶栏：左边返回与新建，中间画板名，右边保存状态。
pub fn top_bar(
    ui: &mut egui::Ui,
    palette: &theme::Palette,
    rect: Rect,
    title: &str,
    state: &str,
    error: bool,
) -> TopBarOutput {
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::ZERO, palette.canvas);
    painter.line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, palette.edge),
    );

    let inner = rect.shrink2(Vec2::new(theme::HINT_INSET, 5.0));
    let mut output = TopBarOutput {
        home: false,
        new_kind: None,
    };
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.horizontal_centered(|ui| {
            if ui.button(strings::BACK_TO_HOME).clicked() {
                output.home = true;
            }
            let picked = ui
                .menu_button(strings::NEW_MENU, |ui| {
                    let mut picked = None;
                    for (label, kind) in [
                        (strings::NEW_TEXT, BlockKind::Text),
                        (strings::NEW_TABLE, BlockKind::Table),
                        (strings::NEW_SHAPE, BlockKind::Shape),
                        (strings::NEW_HIGHLIGHT, BlockKind::Highlight),
                    ] {
                        if ui.button(label).clicked() {
                            picked = Some(kind);
                            ui.close();
                        }
                    }
                    picked
                })
                .inner
                .flatten();
            if picked.is_some() {
                output.new_kind = picked;
            }

            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(title)
                    .color(palette.text)
                    .size(theme::BLOCK_FONT),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !state.is_empty() {
                    let colour = if error { palette.danger } else { palette.muted };
                    ui.label(
                        egui::RichText::new(state)
                            .color(colour)
                            .size(theme::SMALL_FONT),
                    );
                }
            });
        });
    });
    output
}

/// 选字体要用的状态，由界面持有。
pub struct FontPicker<'a> {
    pub library: &'a mut Option<FontLibrary>,
    pub installed: &'a mut HashSet<String>,
    pub filter: &'a mut String,
}

/// 操作条上按一下要做的事。画布拿到之后统一走撤销栈执行。
#[derive(Clone, Debug)]
pub enum Command {
    StepTextSize(i32),
    SetTextColour(Option<[u8; 3]>),
    SetBackground(Option<[u8; 3]>),
    SetFont(Option<String>),
    SetImageFit(ImageFit),
    AddTableRow,
    AddTableCol,
    SetShapeKind(ShapeKind),
    SetShapeFill(Option<[u8; 3]>),
    SetShapeStroke(Option<[u8; 3]>),
    SetHighlightColour([u8; 3]),
    SetHighlightOpacity(f32),
    Align(Alignment),
    Distribute(Axis),
    LayerFront,
    LayerBack,
    Lock(bool),
    Show(bool),
    Duplicate,
    Delete,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColourTarget {
    Text,
    Background,
    ShapeFill,
    ShapeStroke,
    Highlight,
}

enum Item {
    Action {
        label: &'static str,
        command: Command,
        active: bool,
        danger: bool,
    },
    Step {
        label: &'static str,
        step: i32,
    },
    Colour {
        target: ColourTarget,
        current: Option<[u8; 3]>,
    },
    Font {
        current: Option<String>,
    },
    Opacity {
        current: f32,
    },
}

impl Item {
    fn width(&self) -> f32 {
        match self {
            Self::Action { label, .. } => label_width(label),
            Self::Step { .. } => 28.0,
            Self::Colour { .. } => 28.0,
            Self::Font { .. } => 86.0,
            Self::Opacity { .. } => 104.0,
        }
    }
}

fn label_width(label: &str) -> f32 {
    let text: f32 = label
        .chars()
        .map(|character| if character.is_ascii() { 7.0 } else { 12.0 })
        .sum();
    (text + 12.0).max(28.0)
}

fn action(label: &'static str, command: Command) -> Item {
    Item::Action {
        label,
        command,
        active: false,
        danger: false,
    }
}

/// 选中块上方的操作条。返回这次按出来的动作，顺序就是按下的顺序。
pub fn selection_toolbar(
    ui: &mut egui::Ui,
    palette: &theme::Palette,
    document: &Document,
    selected: &[u64],
    anchor: Rect,
    area: Rect,
    picker: &mut FontPicker<'_>,
) -> Vec<Command> {
    let entries: Vec<&Entry> = selected
        .iter()
        .filter_map(|id| document.entry(*id))
        .collect();
    if entries.is_empty() {
        return Vec::new();
    }

    let mut items = Vec::new();
    if entries.len() == 1 {
        single_block_items(&mut items, document, entries[0]);
    } else {
        for (label, alignment) in [
            (strings::ALIGN_LEFT, Alignment::Left),
            (strings::ALIGN_CENTER_X, Alignment::CenterX),
            (strings::ALIGN_RIGHT, Alignment::Right),
            (strings::ALIGN_TOP, Alignment::Top),
            (strings::ALIGN_CENTER_Y, Alignment::CenterY),
            (strings::ALIGN_BOTTOM, Alignment::Bottom),
        ] {
            items.push(action(label, Command::Align(alignment)));
        }
        for (label, axis) in [
            (strings::DISTRIBUTE_H, Axis::Horizontal),
            (strings::DISTRIBUTE_V, Axis::Vertical),
        ] {
            items.push(action(label, Command::Distribute(axis)));
        }
    }

    items.push(action(strings::LAYER_FRONT, Command::LayerFront));
    items.push(action(strings::LAYER_BACK, Command::LayerBack));
    let all_locked = entries.iter().all(|entry| entry.locked);
    items.push(action(
        if all_locked {
            strings::UNLOCK
        } else {
            strings::LOCK
        },
        Command::Lock(!all_locked),
    ));
    let any_hidden = entries.iter().any(|entry| !entry.visible);
    items.push(action(
        if any_hidden {
            strings::SHOW
        } else {
            strings::HIDE
        },
        Command::Show(any_hidden),
    ));
    items.push(action(strings::DUPLICATE, Command::Duplicate));
    items.push(Item::Action {
        label: strings::DELETE,
        command: Command::Delete,
        active: false,
        danger: true,
    });

    draw_items(ui, palette, items, anchor, area, picker)
}

fn single_block_items(items: &mut Vec<Item>, document: &Document, entry: &Entry) {
    match entry.kind {
        BlockKind::Text => {
            let Some(Body::Text(text)) = document.body(entry.id) else {
                return;
            };
            items.push(Item::Step {
                label: "A−",
                step: -1,
            });
            items.push(Item::Step {
                label: "A+",
                step: 1,
            });
            items.push(Item::Colour {
                target: ColourTarget::Text,
                current: text.color,
            });
            items.push(Item::Colour {
                target: ColourTarget::Background,
                current: text.background,
            });
            items.push(Item::Font {
                current: text.font.clone(),
            });
        }
        BlockKind::Image => {
            let Some(Body::Image(image)) = document.body(entry.id) else {
                return;
            };
            for (label, fit) in [
                (strings::FIT_COVER, ImageFit::Cover),
                (strings::FIT_CONTAIN, ImageFit::Contain),
                (strings::FIT_FILL, ImageFit::Fill),
            ] {
                items.push(Item::Action {
                    label,
                    command: Command::SetImageFit(fit),
                    active: image.fit == fit,
                    danger: false,
                });
            }
        }
        BlockKind::Table => {
            items.push(action(strings::TABLE_ADD_ROW, Command::AddTableRow));
            items.push(action(strings::TABLE_ADD_COL, Command::AddTableCol));
        }
        BlockKind::Shape => {
            let Some(Body::Shape(shape)) = document.body(entry.id) else {
                return;
            };
            for (label, kind) in [
                (strings::SHAPE_RECT, ShapeKind::Rect),
                (strings::SHAPE_ELLIPSE, ShapeKind::Ellipse),
                (strings::SHAPE_LINE, ShapeKind::Line),
            ] {
                items.push(Item::Action {
                    label,
                    command: Command::SetShapeKind(kind),
                    active: shape.shape == kind,
                    danger: false,
                });
            }
            items.push(Item::Colour {
                target: ColourTarget::ShapeFill,
                current: shape.fill,
            });
            items.push(Item::Colour {
                target: ColourTarget::ShapeStroke,
                current: shape.stroke,
            });
        }
        BlockKind::Highlight => {
            let Some(Body::Highlight(highlight)) = document.body(entry.id) else {
                return;
            };
            items.push(Item::Colour {
                target: ColourTarget::Highlight,
                current: Some(highlight.color),
            });
            items.push(Item::Opacity {
                current: highlight.opacity,
            });
        }
        BlockKind::Video | BlockKind::Audio => {}
    }
}

fn draw_items(
    ui: &mut egui::Ui,
    palette: &theme::Palette,
    items: Vec<Item>,
    anchor: Rect,
    area: Rect,
    picker: &mut FontPicker<'_>,
) -> Vec<Command> {
    let mut rects = Vec::with_capacity(items.len());
    let mut row = 0usize;
    let mut x = 0.0;
    let mut width: f32 = 0.0;
    for item in &items {
        let item_width = item.width();
        if x > 0.0 && x + item_width > MAX_ROW_WIDTH {
            row += 1;
            x = 0.0;
        }
        rects.push(Rect::from_min_size(
            Pos2::new(x, row as f32 * (TOOLBAR_ROW_HEIGHT + TOOLBAR_ROW_GAP)),
            Vec2::new(item_width, TOOLBAR_ROW_HEIGHT),
        ));
        x += item_width + TOOLBAR_ITEM_GAP;
        width = width.max(x - TOOLBAR_ITEM_GAP);
    }
    let rows = row + 1;
    let bar_width = width + BAR_PAD * 2.0;
    let bar_height =
        rows as f32 * TOOLBAR_ROW_HEIGHT + (rows - 1) as f32 * TOOLBAR_ROW_GAP + BAR_PAD * 2.0;
    let (left, top) = bar_position(anchor, area, bar_width, bar_height);
    let bar = Rect::from_min_size(Pos2::new(left, top), Vec2::new(bar_width, bar_height));

    // 取一份自己的 painter：下面要 `ui.put` 放滑杆，不能一直借着 ui。
    let painter = ui.painter().clone();
    painter.rect(
        bar,
        CornerRadius::same(RADIUS),
        palette.block_fill,
        Stroke::new(1.0, palette.edge),
        StrokeKind::Inside,
    );

    let mut commands = Vec::new();
    for (index, (item, rect)) in items.iter().zip(rects).enumerate() {
        let rect = rect.translate(bar.min.to_vec2() + Vec2::splat(BAR_PAD));
        match item {
            Item::Action {
                label,
                command,
                active,
                danger,
            } => {
                let colour = if *danger {
                    palette.danger
                } else {
                    palette.text
                };
                let response = toolbar_button(ui, &painter, palette, rect, label, *active, colour);
                if response.clicked() {
                    commands.push(command.clone());
                }
            }
            Item::Step { label, step } => {
                let response =
                    toolbar_button(ui, &painter, palette, rect, label, false, palette.text);
                if response.clicked() {
                    commands.push(Command::StepTextSize(*step));
                }
            }
            Item::Colour { target, current } => {
                let response = colour_button(ui, &painter, palette, rect, index, *target, *current);
                let response = match target {
                    ColourTarget::ShapeFill => response.on_hover_text(strings::SHAPE_FILL),
                    ColourTarget::ShapeStroke => response.on_hover_text(strings::SHAPE_STROKE),
                    _ => response,
                };
                let mut value = *current;
                if colour_popup(palette, &response, &mut value)
                    && let Some(command) = colour_command(*target, value)
                {
                    commands.push(command);
                }
            }
            Item::Font { current } => {
                let label = current.as_deref().unwrap_or(strings::FONT_DEFAULT);
                let elided = elide(label, 8);
                let response =
                    toolbar_button(ui, &painter, palette, rect, &elided, false, palette.text);
                if response.clicked() && picker.library.is_none() {
                    // 枚举系统字体要几百毫秒，只在这一下花，不放在启动路径上。
                    *picker.library = Some(FontLibrary::load());
                }
                let mut value = current.clone();
                if font_popup(&response, &mut value, picker) {
                    commands.push(Command::SetFont(value));
                }
            }
            Item::Opacity { current } => {
                let mut value = *current;
                let response = ui.put(
                    rect,
                    egui::Slider::new(&mut value, 0.0..=1.0).show_value(true),
                );
                if response.changed() {
                    commands.push(Command::SetHighlightOpacity(value));
                }
                response.on_hover_text(strings::HIGHLIGHT_OPACITY);
            }
        }
    }
    commands
}

fn colour_command(target: ColourTarget, value: Option<[u8; 3]>) -> Option<Command> {
    match target {
        ColourTarget::Text => Some(Command::SetTextColour(value)),
        ColourTarget::Background => Some(Command::SetBackground(value)),
        ColourTarget::ShapeFill => Some(Command::SetShapeFill(value)),
        ColourTarget::ShapeStroke => Some(Command::SetShapeStroke(value)),
        ColourTarget::Highlight => value.map(Command::SetHighlightColour),
    }
}

fn bar_position(anchor: Rect, area: Rect, width: f32, height: f32) -> (f32, f32) {
    let min_top = area.top() + 4.0;
    let max_top = (area.bottom() - height - 4.0).max(min_top);
    let above = anchor.top() - TOOLBAR_GAP - height;
    let top = if above >= min_top {
        above
    } else {
        anchor.bottom() + TOOLBAR_GAP
    };
    let min_left = area.left() + 4.0;
    let max_left = (area.right() - width - 4.0).max(min_left);
    let left = (anchor.center().x - width / 2.0).clamp(min_left, max_left);
    (left, top.clamp(min_top, max_top))
}

fn toolbar_button(
    ui: &egui::Ui,
    painter: &egui::Painter,
    palette: &theme::Palette,
    rect: Rect,
    label: &str,
    active: bool,
    colour: Color32,
) -> egui::Response {
    // 操作条里每条的文案都不重复，拿文案当 id 就够稳定。
    let response = ui.interact(
        rect,
        ui.scope_id().with(("toolbar-button", label)),
        Sense::click(),
    );
    if active || response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(RADIUS_SMALL), palette.media_fill);
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(theme::SMALL_FONT),
        colour,
    );
    response
}

fn colour_button(
    ui: &egui::Ui,
    painter: &egui::Painter,
    palette: &theme::Palette,
    rect: Rect,
    index: usize,
    target: ColourTarget,
    current: Option<[u8; 3]>,
) -> egui::Response {
    let response = ui.interact(
        rect,
        ui.scope_id().with(("toolbar-colour", index)),
        Sense::click(),
    );
    if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(RADIUS_SMALL), palette.media_fill);
    }
    paint_colour_swatch(painter, palette, rect.center(), target, current);
    response
}

fn paint_colour_swatch(
    painter: &egui::Painter,
    palette: &theme::Palette,
    centre: Pos2,
    target: ColourTarget,
    current: Option<[u8; 3]>,
) {
    let colour = current.map(|[r, g, b]| Color32::from_rgb(r, g, b));
    match target {
        ColourTarget::Text | ColourTarget::Highlight => match colour {
            Some(colour) => {
                painter.circle_filled(centre, 7.0, colour);
            }
            None => {
                painter.circle_stroke(centre, 7.0, Stroke::new(1.0, palette.text));
            }
        },
        ColourTarget::Background | ColourTarget::ShapeFill => {
            let square = Rect::from_center_size(centre, Vec2::splat(14.0));
            match colour {
                Some(colour) => {
                    painter.rect_filled(square, CornerRadius::same(2), colour);
                }
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
        ColourTarget::ShapeStroke => {
            let square = Rect::from_center_size(centre, Vec2::splat(13.0));
            let stroke = match colour {
                Some(colour) => Stroke::new(3.0, colour),
                None => Stroke::new(1.0, palette.text),
            };
            painter.rect_stroke(square, CornerRadius::same(2), stroke, StrokeKind::Inside);
        }
    }
}

/// 按钮上放不下整个字体名，截断加省略号。
fn elide(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let kept: String = text.chars().take(limit.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// 字体列表：默认 + 系统里装好的字体族，带一个过滤框。
fn font_popup(
    button: &egui::Response,
    font: &mut Option<String>,
    picker: &mut FontPicker<'_>,
) -> bool {
    let mut changed = false;
    egui::Popup::from_toggle_button_response(button)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(240.0);
            if ui
                .selectable_label(font.is_none(), strings::FONT_DEFAULT)
                .clicked()
            {
                *font = None;
                changed = true;
                ui.close();
            }

            ui.add(egui::TextEdit::singleline(picker.filter).hint_text(strings::FONT_FILTER));
            let FontPicker {
                library,
                installed,
                filter,
            } = picker;
            let Some(library) = library.as_ref() else {
                return;
            };
            let needle = filter.to_lowercase();
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for (index, name) in library.names().iter().enumerate() {
                        if !needle.is_empty() && !name.to_lowercase().contains(&needle) {
                            continue;
                        }
                        if ui.selectable_label(false, name).clicked()
                            && library.install(ui.ctx(), index).is_some()
                        {
                            installed.insert(name.clone());
                            *font = Some(name.clone());
                            changed = true;
                            ui.close();
                        }
                    }
                });
        });
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
                    let swatch = Rect::from_min_size(ui.cursor().min, Vec2::splat(22.0));
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

/// 画布要按一份文本样式改字号时用：在档位里上下走一格。
pub fn stepped_size(current: Option<f32>, step: i32) -> Option<f32> {
    let sizes = theme::TEXT_SIZES;
    let current = current.unwrap_or(theme::BLOCK_FONT);
    let index = sizes
        .iter()
        .position(|size| (size - current).abs() < 0.5)
        .unwrap_or(2);
    let next = (index as i32 + step).clamp(0, sizes.len() as i32 - 1) as usize;
    Some(sizes[next])
}

/// 文本块的文字颜色：用户设过就用他设的；只配了底色就按底色亮度挑黑或白，
/// 深色主题下的白字落在浅黄底上根本看不见，这种时候得替用户选一次。
pub fn text_colour(text: &TextBlock, palette: &theme::Palette) -> Color32 {
    if let Some([r, g, b]) = text.color {
        return Color32::from_rgb(r, g, b);
    }
    if let Some([r, g, b]) = text.background {
        let luma = (0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b)) / 255.0;
        return if luma > 0.55 {
            Color32::BLACK
        } else {
            Color32::WHITE
        };
    }
    palette.text
}
