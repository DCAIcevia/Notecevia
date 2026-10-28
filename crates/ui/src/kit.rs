#![forbid(unsafe_code)]

//! 界面组件层：尺寸只在这里定，状态只在这里写一遍。
//!
//! 三个层次：
//! - 档位：行高、内缩、图标框这些结构数字集中在这里；颜色与字号仍取主题。
//! - 布局：`Rows` 一行一行往下给区域，视图里不再手算 `Rect::from_*`。
//! - 控件：行、文字按钮、筛选片、输入框，悬停 / 按下 / 选中 / 焦点的表现只实现一次。
//!
//! 判断标准很简单：视图里出现裸数字或者自己 `ui.interact`，就是又绕过了这一层。

use egui::{
    Align2, Button, CornerRadius, FontId, Rect, Response, RichText, Sense, TextEdit, Ui, Vec2,
};

use crate::icons::{self, Icon};
use crate::theme;

/// 列表行高。笔记、回收站、资源栏的行都用它。
pub const ROW: f32 = 40.0;
/// 工具栏那一行的行高（筛选条、面板头部里放控件的地方）。
pub const TOOL_ROW: f32 = 32.0;
/// 面板内容的内缩。左右都用它，别在视图里写 8 和 12。
pub const INSET: f32 = 12.0;
/// 行与行之间不留空隙：靠悬停底色分界，不靠留白。
pub const GAP: f32 = 0.0;
/// 行里图标的框宽。图标画在框中间，文字从框后面起。
pub const ICON_BOX: f32 = 24.0;

/// 竖向排一列：一行一行往下给区域，放不下就停。
pub struct Rows {
    left: f32,
    width: f32,
    cursor: f32,
    bottom: f32,
    height: f32,
    gap: f32,
}

impl Rows {
    pub fn new(rect: Rect, height: f32) -> Self {
        Self {
            left: rect.min.x,
            width: rect.width(),
            cursor: rect.min.y,
            bottom: rect.max.y,
            height,
            gap: GAP,
        }
    }

    /// 下一行；余下的高度不够一整行就返回 None。
    pub fn take(&mut self) -> Option<Rect> {
        if self.cursor + self.height > self.bottom {
            return None;
        }
        let row = Rect::from_min_size(
            egui::pos2(self.left, self.cursor),
            Vec2::new(self.width, self.height),
        );
        self.cursor += self.height + self.gap;
        Some(row)
    }
}

/// 一行要摆什么。
pub struct RowSpec<'a> {
    pub icon: Option<Icon>,
    pub label: &'a str,
    /// 主文右侧的灰字：大小、剩余天数这类。
    pub meta: &'a str,
    /// 右端的一串文字动作。
    pub actions: &'a [&'a str],
    /// 危险动作的下标：那一颗用 danger 色。
    pub danger: Option<usize>,
    pub selected: bool,
}

/// 一行按下来的结果。
#[derive(Default)]
pub struct RowHit {
    /// 按在行身上（不是动作）。
    pub row: bool,
    /// 按在哪个动作上。
    pub action: Option<usize>,
}

/// 画一行：悬停有底，选中也有底，键盘焦点有框。返回按了什么。
pub fn row(
    ui: &mut Ui,
    rect: Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    spec: &RowSpec<'_>,
) -> RowHit {
    let mut hit = RowHit::default();
    let painter = ui.painter().clone();
    let chrome = theme::chrome();
    let tokens = theme::ui();
    let response = ui.interact(rect, ui.scope_id().with(id), Sense::click());
    let focused = response.has_focus();
    if spec.selected {
        painter.rect_filled(rect, CornerRadius::ZERO, chrome.selected);
    } else if response.hovered() || focused {
        painter.rect_filled(rect, CornerRadius::ZERO, chrome.hover);
    }
    if focused {
        painter.rect_stroke(
            rect.shrink(1.0),
            CornerRadius::ZERO,
            egui::Stroke::new(1.0, chrome.text_muted),
            egui::StrokeKind::Inside,
        );
    }
    let mut left = rect.min.x + INSET;
    if let Some(icon) = spec.icon {
        icons::draw(
            &painter,
            Rect::from_center_size(
                egui::pos2(left + ICON_BOX / 2.0 - 2.0, rect.center().y),
                Vec2::splat(14.0),
            ),
            icon,
            chrome.text_faint,
        );
        left += ICON_BOX;
    }
    // 右端先摆动作，剩下的宽度才是文字的。
    let mut actions_left = rect.max.x - INSET;
    let mut action_rects = Vec::new();
    for (index, label) in spec.actions.iter().enumerate().rev() {
        let width = label_width(ui, label, tokens.meta);
        actions_left -= width;
        action_rects.push((
            index,
            Rect::from_center_size(
                egui::pos2(actions_left + width / 2.0, rect.center().y),
                Vec2::new(width, TOOL_ROW),
            ),
        ));
        actions_left -= tokens.gap_m;
    }
    let text_right = if action_rects.is_empty() {
        rect.max.x - INSET
    } else {
        actions_left
    };
    if !spec.meta.is_empty() {
        let meta_width = label_width(ui, spec.meta, tokens.meta);
        painter.text(
            egui::pos2(text_right, rect.center().y),
            Align2::RIGHT_CENTER,
            spec.meta,
            FontId::proportional(tokens.meta),
            chrome.text_faint,
        );
        let text_right = text_right - meta_width - tokens.gap_m;
        let area = Rect::from_min_max(
            egui::pos2(left, rect.min.y),
            egui::pos2(text_right, rect.max.y),
        );
        painter.with_clip_rect(area).text(
            egui::pos2(left, rect.center().y),
            Align2::LEFT_CENTER,
            spec.label,
            FontId::proportional(tokens.body),
            chrome.text,
        );
    } else {
        let area = Rect::from_min_max(
            egui::pos2(left, rect.min.y),
            egui::pos2(text_right, rect.max.y),
        );
        painter.with_clip_rect(area).text(
            egui::pos2(left, rect.center().y),
            Align2::LEFT_CENTER,
            spec.label,
            FontId::proportional(tokens.body),
            chrome.text,
        );
    }
    for (index, action_rect) in action_rects {
        let danger = spec.danger == Some(index);
        let tone = if danger { Tone::Danger } else { Tone::Quiet };
        if text_button(ui, action_rect, spec.actions[index], tone).clicked() {
            hit.action = Some(index);
        }
    }
    if response.clicked() && hit.action.is_none() {
        hit.row = true;
    }
    hit
}

/// 文字按钮的语气。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// 次要动作：灰色，悬停变深。
    Quiet,
    /// 危险动作：一直用 danger 色。
    Danger,
}

/// 文字按钮：没有底色，只换字色；键盘焦点画框。
pub fn text_button(ui: &mut Ui, rect: Rect, label: &str, tone: Tone) -> Response {
    let chrome = theme::chrome();
    let colour = match tone {
        Tone::Quiet => chrome.text_muted,
        Tone::Danger => chrome.danger,
    };
    let response = ui.put(
        rect,
        Button::new(RichText::new(label).size(theme::ui().meta).color(colour)).frame(false),
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            CornerRadius::same(theme::ui().radius_control),
            egui::Stroke::new(1.0, chrome.text_muted),
            egui::StrokeKind::Inside,
        );
    }
    response
}

/// 筛选片：选中有底色，未选中只有字；悬停只换底。
pub fn chip(ui: &mut Ui, rect: Rect, label: &str, selected: bool) -> Response {
    let chrome = theme::chrome();
    let tokens = theme::ui();
    let response = ui.interact(rect, ui.scope_id().with(label), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect_filled(
            rect,
            CornerRadius::same(tokens.radius_control),
            chrome.pressed,
        );
    } else if response.hovered() {
        painter.rect_filled(
            rect,
            CornerRadius::same(tokens.radius_control),
            chrome.hover,
        );
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(1.0),
            CornerRadius::same(tokens.radius_control),
            egui::Stroke::new(1.0, chrome.text_muted),
            egui::StrokeKind::Inside,
        );
    }
    let colour = if selected || response.hovered() {
        chrome.text
    } else {
        chrome.text_muted
    };
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(tokens.meta),
        colour,
    );
    response
}

/// 输入框：自己画底，里面放一个无框的 TextEdit。
pub fn field(ui: &mut Ui, rect: Rect, text: &mut String, hint: &str) {
    let chrome = theme::chrome();
    let tokens = theme::ui();
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(tokens.radius_control),
        chrome.field,
    );
    ui.put(
        rect.shrink2(Vec2::new(tokens.gap_m, 0.0)),
        TextEdit::singleline(text)
            .hint_text(hint)
            .frame(egui::Frame::NONE)
            .font(FontId::proportional(tokens.meta + 0.5)),
    );
}

/// 文字宽度：布局要按它先分空间。
fn label_width(ui: &Ui, label: &str, size: f32) -> f32 {
    ui.painter()
        .layout_no_wrap(
            label.to_owned(),
            FontId::proportional(size),
            theme::chrome().text,
        )
        .size()
        .x
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect {
        min: egui::pos2(0.0, 100.0),
        max: egui::pos2(200.0, 100.0 + 3.0 * ROW),
    };

    #[test]
    fn rows_stop_when_the_space_runs_out() {
        let mut rows = Rows::new(AREA, ROW);
        assert_eq!(rows.take().map(|row| row.min.y), Some(100.0));
        assert_eq!(rows.take().map(|row| row.min.y), Some(100.0 + ROW));
        assert_eq!(rows.take().map(|row| row.min.y), Some(100.0 + ROW * 2.0));
        assert!(rows.take().is_none(), "放不下就不给，调用方别再画");
    }

    #[test]
    fn rows_keep_the_width_of_the_area() {
        let mut rows = Rows::new(AREA, ROW);
        let row = rows.take().expect("第一行");
        assert_eq!(row.width(), AREA.width());
        assert_eq!(row.min.x, AREA.min.x);
    }
}
