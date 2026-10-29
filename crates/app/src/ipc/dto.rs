//! 前端与内核之间的数据形状。
//!
//! 内核类型不带 serde：盘上的格式只认 CBOR，序列化形状属于窗口壳这一层。
//! 这里做双向映射，前端拿到的是稳定的 camelCase 名字；字段增删改都在这里收口。

use notecevia_core::{
    BlockKind, Board, Body, Document, Entry, Geometry, HighlightBlock, ImageBlock, ImageFit,
    MediaBlock, ShapeBlock, ShapeKind, Shelf, TableBlock, TextBlock, Volume,
};
use notecevia_store::{
    BoardInfo, MediaInfo, SearchHit, ShelfInfo, TrashItem, TrashKind, VolumeInfo, days_left,
};
use serde::{Deserialize, Serialize};

/// 世界坐标里的矩形。
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeometryDto {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// 绕块中心顺时针旋转的弧度，省略按 0。
    #[serde(default)]
    pub rotation: f32,
}

impl From<Geometry> for GeometryDto {
    fn from(geometry: Geometry) -> Self {
        Self {
            x: geometry.x,
            y: geometry.y,
            width: geometry.width,
            height: geometry.height,
            rotation: geometry.rotation,
        }
    }
}

impl From<GeometryDto> for Geometry {
    fn from(geometry: GeometryDto) -> Self {
        Self {
            x: geometry.x,
            y: geometry.y,
            width: geometry.width,
            height: geometry.height,
            rotation: geometry.rotation,
        }
    }
}

/// 块种类。编号与盘上的值无关，前端只认名字。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockKindDto {
    Text,
    Image,
    Video,
    Audio,
    Table,
    Shape,
    Highlight,
}

impl From<BlockKind> for BlockKindDto {
    fn from(kind: BlockKind) -> Self {
        match kind {
            BlockKind::Text => Self::Text,
            BlockKind::Image => Self::Image,
            BlockKind::Video => Self::Video,
            BlockKind::Audio => Self::Audio,
            BlockKind::Table => Self::Table,
            BlockKind::Shape => Self::Shape,
            BlockKind::Highlight => Self::Highlight,
        }
    }
}

impl From<BlockKindDto> for BlockKind {
    fn from(kind: BlockKindDto) -> Self {
        match kind {
            BlockKindDto::Text => Self::Text,
            BlockKindDto::Image => Self::Image,
            BlockKindDto::Video => Self::Video,
            BlockKindDto::Audio => Self::Audio,
            BlockKindDto::Table => Self::Table,
            BlockKindDto::Shape => Self::Shape,
            BlockKindDto::Highlight => Self::Highlight,
        }
    }
}

/// 目录项：位置、层级、指向哪一块。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryDto {
    pub id: u64,
    pub kind: BlockKindDto,
    pub geometry: GeometryDto,
    /// 层级：大的画在上面。省略按 0。
    #[serde(default)]
    pub z: i32,
    /// 锁住后不能拖动、缩放、旋转。省略按 false。
    #[serde(default)]
    pub locked: bool,
    /// 隐藏后不画、不响应点击。省略按 true。
    #[serde(default = "default_visible")]
    pub visible: bool,
}

fn default_visible() -> bool {
    true
}

impl From<&Entry> for EntryDto {
    fn from(entry: &Entry) -> Self {
        Self {
            id: entry.id,
            kind: entry.kind.into(),
            geometry: entry.geometry.into(),
            z: entry.z,
            locked: entry.locked,
            visible: entry.visible,
        }
    }
}

impl From<EntryDto> for Entry {
    fn from(entry: EntryDto) -> Self {
        Self {
            id: entry.id,
            kind: entry.kind.into(),
            geometry: entry.geometry.into(),
            z: entry.z,
            locked: entry.locked,
            visible: entry.visible,
        }
    }
}

/// 图片填充方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageFitDto {
    Cover,
    Contain,
    Fill,
}

impl From<ImageFit> for ImageFitDto {
    fn from(fit: ImageFit) -> Self {
        match fit {
            ImageFit::Cover => Self::Cover,
            ImageFit::Contain => Self::Contain,
            ImageFit::Fill => Self::Fill,
        }
    }
}

impl From<ImageFitDto> for ImageFit {
    fn from(fit: ImageFitDto) -> Self {
        match fit {
            ImageFitDto::Cover => Self::Cover,
            ImageFitDto::Contain => Self::Contain,
            ImageFitDto::Fill => Self::Fill,
        }
    }
}

/// 形状种类。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShapeKindDto {
    Rect,
    Ellipse,
    Line,
}

impl From<ShapeKind> for ShapeKindDto {
    fn from(shape: ShapeKind) -> Self {
        match shape {
            ShapeKind::Rect => Self::Rect,
            ShapeKind::Ellipse => Self::Ellipse,
            ShapeKind::Line => Self::Line,
        }
    }
}

impl From<ShapeKindDto> for ShapeKind {
    fn from(shape: ShapeKindDto) -> Self {
        match shape {
            ShapeKindDto::Rect => Self::Rect,
            ShapeKindDto::Ellipse => Self::Ellipse,
            ShapeKindDto::Line => Self::Line,
        }
    }
}

/// 块的正文。`kind` 是标签；视频与音频共用一个 `media` 正文，种类由目录项决定。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BodyDto {
    Text {
        id: u64,
        text: String,
        #[serde(default)]
        font: Option<String>,
        #[serde(default)]
        size: Option<f32>,
        #[serde(default)]
        color: Option<[u8; 3]>,
        #[serde(default)]
        background: Option<[u8; 3]>,
    },
    Image {
        id: u64,
        media: String,
        #[serde(default)]
        name: String,
        fit: ImageFitDto,
    },
    Media {
        id: u64,
        media: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        bytes: u64,
    },
    Table {
        id: u64,
        rows: u16,
        cols: u16,
        cells: Vec<String>,
    },
    Shape {
        id: u64,
        shape: ShapeKindDto,
        #[serde(default)]
        fill: Option<[u8; 3]>,
        #[serde(default)]
        stroke: Option<[u8; 3]>,
        stroke_width: f32,
    },
    Highlight {
        id: u64,
        color: [u8; 3],
        opacity: f32,
    },
}

impl BodyDto {
    /// 这条正文属于哪一块。
    pub fn id(&self) -> u64 {
        match self {
            Self::Text { id, .. }
            | Self::Image { id, .. }
            | Self::Media { id, .. }
            | Self::Table { id, .. }
            | Self::Shape { id, .. }
            | Self::Highlight { id, .. } => *id,
        }
    }

    fn from_body(id: u64, body: &Body) -> Self {
        match body {
            Body::Text(text) => Self::Text {
                id,
                text: text.text.clone(),
                font: text.font.clone(),
                size: text.size,
                color: text.color,
                background: text.background,
            },
            Body::Image(image) => Self::Image {
                id,
                media: image.media.clone(),
                name: image.name.clone(),
                fit: image.fit.into(),
            },
            Body::Media(media) => Self::Media {
                id,
                media: media.media.clone(),
                name: media.name.clone(),
                bytes: media.bytes,
            },
            Body::Table(table) => Self::Table {
                id,
                rows: table.rows,
                cols: table.cols,
                cells: table.cells.clone(),
            },
            Body::Shape(shape) => Self::Shape {
                id,
                shape: shape.shape.into(),
                fill: shape.fill,
                stroke: shape.stroke,
                stroke_width: shape.stroke_width,
            },
            Body::Highlight(highlight) => Self::Highlight {
                id,
                color: highlight.color,
                opacity: highlight.opacity,
            },
        }
    }
}

impl From<BodyDto> for Body {
    fn from(body: BodyDto) -> Self {
        match body {
            BodyDto::Text {
                text,
                font,
                size,
                color,
                background,
                ..
            } => Self::Text(TextBlock {
                text,
                font,
                size,
                color,
                background,
            }),
            BodyDto::Image {
                media, name, fit, ..
            } => Self::Image(ImageBlock {
                media,
                name,
                fit: fit.into(),
            }),
            BodyDto::Media {
                media, name, bytes, ..
            } => Self::Media(MediaBlock { media, name, bytes }),
            BodyDto::Table {
                rows, cols, cells, ..
            } => Self::Table(TableBlock { rows, cols, cells }),
            BodyDto::Shape {
                shape,
                fill,
                stroke,
                stroke_width,
                ..
            } => Self::Shape(ShapeBlock {
                shape: shape.into(),
                fill,
                stroke,
                stroke_width,
            }),
            BodyDto::Highlight { color, opacity, .. } => {
                Self::Highlight(HighlightBlock { color, opacity })
            }
        }
    }
}

/// 画板的元信息：标题与标签。目录项单独放。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardMetaDto {
    pub title: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// 一张画板：元信息、目录项与正文分开列，前端按 id 自己拼。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentDto {
    pub board: BoardMetaDto,
    pub entries: Vec<EntryDto>,
    /// 正文按块编号一一对应；目录项可以没有正文，前端按空内容处理。
    pub bodies: Vec<BodyDto>,
}

impl From<&Document> for DocumentDto {
    fn from(document: &Document) -> Self {
        Self {
            board: BoardMetaDto {
                title: document.board.title.clone(),
                tags: document.board.tags.clone(),
            },
            entries: document.board.entries.iter().map(EntryDto::from).collect(),
            bodies: document
                .bodies
                .iter()
                .map(|(&id, body)| BodyDto::from_body(id, body))
                .collect(),
        }
    }
}

impl From<DocumentDto> for Document {
    fn from(document: DocumentDto) -> Self {
        let mut board = Board::new(document.board.title);
        board.tags = document.board.tags;
        board.entries = document.entries.into_iter().map(Entry::from).collect();
        let bodies = document
            .bodies
            .into_iter()
            .map(|body| (body.id(), Body::from(body)))
            .collect();
        Self { board, bodies }
    }
}

/// 列表页的一行。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardSummary {
    pub id: u64,
    pub title: String,
    pub tags: Vec<String>,
    /// Unix 秒。
    pub updated_at: i64,
    pub entry_count: usize,
}

impl From<&BoardInfo> for BoardSummary {
    fn from(info: &BoardInfo) -> Self {
        Self {
            id: info.id,
            title: info.title.clone(),
            tags: info.tags.clone(),
            updated_at: info.updated_at,
            entry_count: info.entry_count,
        }
    }
}

/// 资源库里的一份媒体。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSummary {
    pub hash: String,
    pub kind: BlockKindDto,
    pub name: String,
    pub bytes: u64,
    /// 有几张画板在用它。
    pub boards: usize,
}

impl From<&MediaInfo> for MediaSummary {
    fn from(info: &MediaInfo) -> Self {
        Self {
            hash: info.hash.clone(),
            kind: info.kind.into(),
            name: info.name.clone(),
            bytes: info.bytes,
            boards: info.boards,
        }
    }
}

/// 一条搜索命中。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitDto {
    pub board_id: u64,
    pub title: String,
    pub snippet: String,
}

impl From<&SearchHit> for SearchHitDto {
    fn from(hit: &SearchHit) -> Self {
        Self {
            board_id: hit.board_id,
            title: hit.title.clone(),
            snippet: hit.snippet.clone(),
        }
    }
}

/// 存盘结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    /// 这次写盘用的修改时间，Unix 秒。
    pub updated_at: i64,
}

/// 一本册的列表行。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeSummary {
    pub id: u64,
    pub title: String,
    pub page_count: usize,
    pub updated_at: i64,
}

impl From<&VolumeInfo> for VolumeSummary {
    fn from(info: &VolumeInfo) -> Self {
        Self {
            id: info.id,
            title: info.title.clone(),
            page_count: info.page_count,
            updated_at: info.updated_at,
        }
    }
}

/// 新建一页的结果：页本身与它落进的册。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageCreatedDto {
    pub board: BoardSummary,
    pub volume: VolumeSummary,
}

/// 一本册：标题与有序的页编号。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeDto {
    pub title: String,
    pub pages: Vec<u64>,
}

impl From<&Volume> for VolumeDto {
    fn from(volume: &Volume) -> Self {
        Self {
            title: volume.title.clone(),
            pages: volume.pages.clone(),
        }
    }
}

impl From<VolumeDto> for Volume {
    fn from(volume: VolumeDto) -> Self {
        Self {
            title: volume.title,
            pages: volume.pages,
        }
    }
}

/// 一层书架的列表行。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfSummary {
    pub id: u64,
    pub title: String,
    pub volume_count: usize,
    pub updated_at: i64,
}

impl From<&ShelfInfo> for ShelfSummary {
    fn from(info: &ShelfInfo) -> Self {
        Self {
            id: info.id,
            title: info.title.clone(),
            volume_count: info.volume_count,
            updated_at: info.updated_at,
        }
    }
}

/// 一层书架：标题与有序的册编号。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfDto {
    pub title: String,
    pub volumes: Vec<u64>,
}

impl From<&Shelf> for ShelfDto {
    fn from(shelf: &Shelf) -> Self {
        Self {
            title: shelf.title.clone(),
            volumes: shelf.volumes.clone(),
        }
    }
}

impl From<ShelfDto> for Shelf {
    fn from(shelf: ShelfDto) -> Self {
        Self {
            title: shelf.title,
            volumes: shelf.volumes,
        }
    }
}

/// 回收站条目的种类。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrashKindDto {
    Board,
    Volume,
    Shelf,
}

impl From<TrashKind> for TrashKindDto {
    fn from(kind: TrashKind) -> Self {
        match kind {
            TrashKind::Board => Self::Board,
            TrashKind::Volume => Self::Volume,
            TrashKind::Shelf => Self::Shelf,
        }
    }
}

/// 回收站里的一条。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashItemDto {
    /// `.trash/` 下的目录名，还原与彻底删除都按它定位。
    pub folder: String,
    pub kind: TrashKindDto,
    pub id: u64,
    pub title: String,
    /// Unix 秒。
    pub deleted_at: i64,
    pub bytes: u64,
    /// 还剩几天到期，0 表示已经到期。
    pub days_left: i64,
}

impl TrashItemDto {
    /// 生成一条回收站条目；`days_left` 用当前时间算，前端不用自己算。
    pub fn new(item: &TrashItem, now: i64) -> Self {
        Self {
            folder: item.folder.clone(),
            kind: item.kind.into(),
            id: item.id,
            title: item.title.clone(),
            deleted_at: item.deleted_at,
            bytes: item.bytes,
            days_left: days_left(item.deleted_at, now),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_document() -> Document {
        let mut document = Document::new("测试画板");
        document.board.tags = vec!["标签".to_owned()];

        let text = document.add_text(Geometry::new(10.0, 20.0, 300.0, 80.0));
        let text_body = document.text_mut(text);
        text_body.text = "正文".to_owned();
        text_body.font = Some("Microsoft YaHei".to_owned());
        text_body.size = Some(16.0);
        text_body.color = Some([0x2c, 0x6c, 0xc8]);
        text_body.background = Some([0xff, 0xee, 0xaa]);

        let image = document.add_block(BlockKind::Image, Geometry::new(0.0, 0.0, 100.0, 100.0));
        if let Some(Body::Image(block)) = document.body_mut(image) {
            block.media = "abc".to_owned();
            block.name = "图.png".to_owned();
            block.fit = ImageFit::Contain;
        }

        let video = document.add_block(BlockKind::Video, Geometry::new(0.0, 0.0, 100.0, 100.0));
        if let Some(Body::Media(block)) = document.body_mut(video) {
            block.media = "def".to_owned();
            block.name = "视频.mp4".to_owned();
            block.bytes = 42;
        }

        let table = document.add_block(BlockKind::Table, Geometry::new(0.0, 0.0, 100.0, 100.0));
        if let Some(Body::Table(block)) = document.body_mut(table) {
            block.rows = 2;
            block.cols = 2;
            block.cells = vec![
                "a".to_owned(),
                "b".to_owned(),
                "c".to_owned(),
                "d".to_owned(),
            ];
        }

        let shape = document.add_block(BlockKind::Shape, Geometry::new(0.0, 0.0, 100.0, 100.0));
        if let Some(Body::Shape(block)) = document.body_mut(shape) {
            block.shape = ShapeKind::Ellipse;
            block.stroke_width = 3.0;
        }

        let highlight =
            document.add_block(BlockKind::Highlight, Geometry::new(0.0, 0.0, 100.0, 100.0));
        if let Some(Body::Highlight(block)) = document.body_mut(highlight) {
            block.color = [0xff, 0xe0, 0x66];
            block.opacity = 0.4;
        }

        if let Some(entry) = document.entry_mut(text) {
            entry.z = 5;
            entry.locked = true;
            entry.visible = false;
        }
        document
    }

    #[test]
    fn document_roundtrips_through_dto() {
        let document = full_document();
        let dto = DocumentDto::from(&document);
        let back: Document = dto.into();
        assert_eq!(back, document);
    }

    #[test]
    fn json_uses_camel_case_names() {
        let dto = DocumentDto::from(&full_document());
        let json = serde_json::to_string(&dto).expect("序列化");
        assert!(
            json.contains("\"kind\":\"text\""),
            "正文应该带 kind 标签：{json}"
        );
        assert!(
            json.contains("\"strokeWidth\""),
            "字段应该是 camelCase：{json}"
        );
        assert!(
            json.contains("\"geometry\""),
            "目录项应该带 geometry：{json}"
        );
        let back: DocumentDto = serde_json::from_str(&json).expect("反序列化");
        assert_eq!(back, dto);
    }
}
