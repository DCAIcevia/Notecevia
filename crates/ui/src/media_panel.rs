#![forbid(unsafe_code)]

//! 右侧资源栏：整个工作区的媒体都在这儿，能筛、能搜、能导进来，点一下放进当前笔记。
//!
//! 与画布无关：数据由 app 递进来，按出来的动作交回 app，这里不碰文件也不读盘。

use egui::{Align2, Button, CornerRadius, FontId, Rect, RichText, Sense, TextEdit, Ui, Vec2};
use notecevia_core::BlockKind;

use crate::icons::{self, Icon};
use crate::strings;
use crate::theme;

/// 头部一行：搜索框与导入。
const HEAD: f32 = 52.0;
/// 筛选那一条。
const FILTER: f32 = 36.0;
/// 一行资源。
const ROW: f32 = 30.0;
/// 导入按钮的宽。
const IMPORT: f32 = 52.0;
/// 一个筛选片的宽。
const CHIP: f32 = 62.0;

/// 资源的种类。界面上只认这三种。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    Audio,
}

impl MediaKind {
    /// 块的种类换成资源种类；文字块这类给 None。
    pub fn of(kind: BlockKind) -> Option<Self> {
        match kind {
            BlockKind::Image => Some(Self::Image),
            BlockKind::Video => Some(Self::Video),
            BlockKind::Audio => Some(Self::Audio),
            _ => None,
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Image => Icon::Image,
            Self::Video => Icon::Play,
            Self::Audio => Icon::Audio,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Image => strings::MEDIA_IMAGE,
            Self::Video => strings::MEDIA_VIDEO,
            Self::Audio => strings::MEDIA_AUDIO,
        }
    }
}

/// 资源库里的一条。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaRow {
    /// 内容哈希。放进笔记时用它，不复制字节。
    pub hash: String,
    pub kind: MediaKind,
    pub name: String,
    pub bytes: u64,
    /// 有几张笔记在用它。
    pub boards: usize,
}

/// 资源栏按出来的动作。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MediaAction {
    /// 把这份资源放进当前笔记。
    Insert(MediaRow),
    /// 打开系统文件选择器导进来。
    Import,
}

/// 资源栏自己的状态：搜索词与种类筛选。
pub struct MediaPanel {
    rows: Vec<MediaRow>,
    query: String,
    filter: Option<MediaKind>,
}

impl Default for MediaPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaPanel {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            query: String::new(),
            filter: None,
        }
    }

    /// 换一批资源。搜索词与筛选不跟着重置：换笔记不该打断正在找的东西。
    pub fn set_rows(&mut self, rows: Vec<MediaRow>) {
        self.rows = rows;
    }

    pub fn rows(&self) -> &[MediaRow] {
        &self.rows
    }

    /// 过一遍搜索与筛选之后的条数。
    pub fn visible_len(&self) -> usize {
        self.visible().count()
    }

    fn visible(&self) -> impl Iterator<Item = &MediaRow> {
        let needle = self.query.trim().to_lowercase();
        self.rows.iter().filter(move |row| {
            self.filter.is_none_or(|kind| row.kind == kind)
                && (needle.is_empty() || row.name.to_lowercase().contains(&needle))
        })
    }

    /// 画一栏，返回按出来的动作。
    pub fn ui(&mut self, ui: &mut Ui, rect: Rect) -> Option<MediaAction> {
        let painter = ui.painter().clone();
        let chrome = theme::chrome();
        let tokens = theme::ui();
        let mut action = None;

        // 头部：搜索框在左，导入在右。
        let field = Rect::from_min_size(
            egui::pos2(
                rect.min.x + tokens.gap_m,
                rect.min.y + (HEAD - tokens.control_height) / 2.0,
            ),
            Vec2::new(
                rect.width() - tokens.gap_m * 2.0 - IMPORT,
                tokens.control_height,
            ),
        );
        painter.rect_filled(
            field,
            CornerRadius::same(tokens.radius_control),
            chrome.field,
        );
        ui.put(
            field.shrink2(Vec2::new(tokens.gap_m, 0.0)),
            TextEdit::singleline(&mut self.query)
                .hint_text(strings::MEDIA_SEARCH)
                .frame(egui::Frame::NONE)
                .font(FontId::proportional(tokens.meta + 0.5)),
        );
        let import = Rect::from_min_size(
            egui::pos2(rect.max.x - tokens.gap_m - IMPORT, field.min.y),
            Vec2::new(IMPORT, tokens.control_height),
        );
        if ui
            .put(
                import,
                Button::new(
                    RichText::new(strings::MEDIA_IMPORT)
                        .size(tokens.meta)
                        .color(chrome.text_muted),
                )
                .frame(false),
            )
            .clicked()
        {
            action = Some(MediaAction::Import);
        }
        // 搜索行与筛选行之间不画线：这一栏是一张岛，靠间距分行就够了。

        // 筛选：全部 / 图片 / 视频 / 音频。选中的只换底色与字色，几何不动。
        let mut x = rect.min.x + tokens.gap_m;
        let chips = [
            (None, strings::MEDIA_ALL),
            (Some(MediaKind::Image), strings::MEDIA_IMAGE),
            (Some(MediaKind::Video), strings::MEDIA_VIDEO),
            (Some(MediaKind::Audio), strings::MEDIA_AUDIO),
        ];
        for (kind, label) in chips {
            let chip = Rect::from_min_size(
                egui::pos2(
                    x,
                    rect.min.y + HEAD + (FILTER - tokens.control_height) / 2.0,
                ),
                Vec2::new(CHIP, tokens.control_height),
            );
            let selected = self.filter == kind;
            let response = ui.interact(
                chip,
                ui.scope_id().with(("media-filter", label)),
                Sense::click(),
            );
            if selected {
                painter.rect_filled(
                    chip,
                    CornerRadius::same(tokens.radius_control),
                    chrome.pressed,
                );
            } else if response.hovered() {
                painter.rect_filled(
                    chip,
                    CornerRadius::same(tokens.radius_control),
                    chrome.hover,
                );
            }
            let colour = if selected || response.hovered() {
                chrome.text
            } else {
                chrome.text_muted
            };
            painter.text(
                chip.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(tokens.meta),
                colour,
            );
            if response.clicked() {
                self.filter = kind;
            }
            x += CHIP + tokens.gap_s;
        }

        // 列表。
        let top = rect.min.y + HEAD + FILTER;
        let shown: Vec<MediaRow> = self.visible().cloned().collect();
        if shown.is_empty() {
            let title = if self.rows.is_empty() {
                strings::MEDIA_EMPTY
            } else {
                strings::MEDIA_NO_MATCH
            };
            let anchor = egui::pos2(rect.center().x, top + 48.0);
            painter.text(
                anchor,
                Align2::CENTER_CENTER,
                title,
                FontId::proportional(tokens.body),
                chrome.text_muted,
            );
            return action;
        }
        let mut y = top;
        for row in &shown {
            if y + ROW > rect.max.y {
                break;
            }
            let line = Rect::from_min_size(egui::pos2(rect.min.x, y), Vec2::new(rect.width(), ROW));
            let response = ui.interact(
                line,
                ui.scope_id().with(("media-row", row.hash.as_str())),
                Sense::click(),
            );
            if response.hovered() {
                painter.rect_filled(line, CornerRadius::ZERO, chrome.hover);
            }
            icons::draw(
                &painter,
                Rect::from_center_size(
                    egui::pos2(rect.min.x + tokens.gap_m + 7.0, line.center().y),
                    Vec2::splat(14.0),
                ),
                row.kind.icon(),
                chrome.text_faint,
            );
            let size = strings::size_text(row.bytes);
            let size_width = 56.0_f32;
            let name_area = Rect::from_min_max(
                egui::pos2(rect.min.x + tokens.gap_m + 28.0, line.min.y),
                egui::pos2(
                    rect.max.x - tokens.gap_m - size_width - tokens.gap_s,
                    line.max.y,
                ),
            );
            painter.with_clip_rect(name_area).text(
                egui::pos2(name_area.min.x, line.center().y),
                Align2::LEFT_CENTER,
                if row.name.is_empty() {
                    row.kind.label()
                } else {
                    row.name.as_str()
                },
                FontId::proportional(tokens.meta + 0.5),
                chrome.text_muted,
            );
            painter.text(
                egui::pos2(rect.max.x - tokens.gap_m, line.center().y),
                Align2::RIGHT_CENTER,
                size,
                FontId::proportional(tokens.meta),
                chrome.text_faint,
            );
            if response.clicked() {
                action = Some(MediaAction::Insert(row.clone()));
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            y += ROW;
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str, kind: MediaKind) -> MediaRow {
        MediaRow {
            hash: format!("hash-{name}"),
            kind,
            name: name.to_owned(),
            bytes: 10,
            boards: 1,
        }
    }

    #[test]
    fn the_filter_and_the_search_narrow_the_list() {
        let mut panel = MediaPanel::new();
        panel.set_rows(vec![
            row("猫.png", MediaKind::Image),
            row("狗.mp4", MediaKind::Video),
            row("雨声.mp3", MediaKind::Audio),
        ]);
        assert_eq!(panel.visible_len(), 3);
        panel.filter = Some(MediaKind::Image);
        assert_eq!(panel.visible_len(), 1);
        panel.filter = None;
        panel.query = "狗".to_owned();
        assert_eq!(panel.visible_len(), 1);
        panel.query = "  ".to_owned();
        assert_eq!(panel.visible_len(), 3, "空查询不算条件");
    }
}
