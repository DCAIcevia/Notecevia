//! 格式 2 的落盘与读回：画板文件、块文件在这里和 CBOR 值互相转换。
//!
//! 字段表见 docs/design/format.md。这里只负责读写一致：写出去的值能原样读回来，
//! 读到坏条目时跳过这一条而不是让整张画板打不开。认不出的块类型一律跳过，不猜。

use std::collections::BTreeMap;

use notecevia_core::board::{
    BlockKind, Board, Body, Entry, Geometry, HighlightBlock, ImageBlock, ImageFit, MediaBlock,
    ShapeBlock, ShapeKind, TableBlock, TextBlock,
};

use crate::cbor::Value;
use crate::error::Error;

/// 这一版写的格式号。读到更大的号就停下，让用户升级程序。
pub(crate) const FORMAT: i64 = 2;

/// 从画板文件读出来的东西：目录、修改时间，以及每条目录项自己记的块地址。
pub(crate) struct BoardOnDisk {
    pub board: Board,
    pub updated_at: i64,
    /// 只有文件里真的写了 block_ref 的条目才在这里；没有的由调用方按编号拼。
    pub refs: BTreeMap<u64, String>,
}

impl BoardOnDisk {
    /// 列表页要的概要，不用把正文读进来。
    pub(crate) fn summary(&self) -> Summary {
        Summary {
            title: self.board.title.clone(),
            tags: self.board.tags.clone(),
            updated_at: self.updated_at,
            entry_count: self.board.entries.len(),
        }
    }
}

/// 列表页一行画板的摘要。
pub(crate) struct Summary {
    pub title: String,
    pub tags: Vec<String>,
    pub updated_at: i64,
    pub entry_count: usize,
}

pub(crate) fn board_value(
    board: &Board,
    updated_at: i64,
    reference: impl Fn(u64) -> String,
) -> Value {
    let entries = board
        .entries
        .iter()
        .map(|entry| entry_value(entry, &reference(entry.id)))
        .collect();
    Value::map(vec![
        ("format", Value::Integer(FORMAT)),
        ("kind", Value::text("board")),
        ("title", Value::text(board.title.clone())),
        (
            "tags",
            Value::Array(
                board
                    .tags
                    .iter()
                    .map(|tag| Value::text(tag.clone()))
                    .collect(),
            ),
        ),
        ("updated_at", Value::Integer(updated_at)),
        ("entries", Value::Array(entries)),
    ])
}

fn entry_value(entry: &Entry, reference: &str) -> Value {
    Value::map(vec![
        ("id", Value::Integer(integer_of(entry.id))),
        ("block_type", Value::Integer(entry.kind.number())),
        ("block_ref", Value::text(reference.to_owned())),
        ("x", Value::Float(f64::from(entry.geometry.x))),
        ("y", Value::Float(f64::from(entry.geometry.y))),
        ("w", Value::Float(f64::from(entry.geometry.width))),
        ("h", Value::Float(f64::from(entry.geometry.height))),
        ("rotation", Value::Float(f64::from(entry.geometry.rotation))),
        ("z_index", Value::Integer(i64::from(entry.z))),
        ("locked", Value::Bool(entry.locked)),
        ("visible", Value::Bool(entry.visible)),
    ])
}

pub(crate) fn board_from(value: &Value) -> Result<BoardOnDisk, Error> {
    format_of(value)?;
    let title = value
        .get("title")
        .and_then(Value::as_text)
        .ok_or(Error::MissingField("title"))?;
    let mut board = Board::new(title);
    if let Some(tags) = value.get("tags").and_then(Value::as_array) {
        for tag in tags {
            if let Some(tag) = tag.as_text() {
                board.add_tag(tag);
            }
        }
    }
    let updated_at = value
        .get("updated_at")
        .and_then(Value::as_integer)
        .unwrap_or(0);
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or(Error::MissingField("entries"))?;

    let mut refs = BTreeMap::new();
    for value in entries {
        // 单条目坏了只丢这一条，其余照读：不认识的块类型、坏掉的坐标都跳过。
        let Some(id) = value.get("id").and_then(Value::as_integer) else {
            continue;
        };
        let Ok(id) = u64::try_from(id) else {
            continue;
        };
        let Some(kind) = value
            .get("block_type")
            .and_then(Value::as_integer)
            .and_then(BlockKind::from_number)
        else {
            continue;
        };
        let (Some(x), Some(y), Some(w), Some(h)) = (
            number(value, "x"),
            number(value, "y"),
            number(value, "w"),
            number(value, "h"),
        ) else {
            continue;
        };
        let geometry = Geometry {
            rotation: number(value, "rotation").unwrap_or(0.0),
            ..Geometry::new(x, y, w, h)
        };
        if !geometry.is_valid() {
            continue;
        }
        let z = value
            .get("z_index")
            .and_then(Value::as_integer)
            .and_then(|z| i32::try_from(z).ok())
            .unwrap_or(0);
        let locked = value
            .get("locked")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let visible = value
            .get("visible")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        board.entries.push(Entry {
            id,
            kind,
            geometry,
            z,
            locked,
            visible,
        });
        if let Some(reference) = value.get("block_ref").and_then(Value::as_text) {
            refs.insert(id, reference.to_owned());
        }
    }
    Ok(BoardOnDisk {
        board,
        updated_at,
        refs,
    })
}

/// 块的种类和正文对得上才写。对不上的不写，旧文件在保存后会作为孤儿回收。
pub(crate) fn body_matches(kind: BlockKind, body: &Body) -> bool {
    match (kind, body) {
        (BlockKind::Video | BlockKind::Audio, Body::Media(_)) => true,
        (kind, body) => body.kind() == kind,
    }
}

pub(crate) fn body_value(kind: BlockKind, id: u64, body: &Body) -> Value {
    let mut pairs = vec![
        ("format", Value::Integer(FORMAT)),
        ("block_type", Value::Integer(kind.number())),
        ("id", Value::Integer(integer_of(id))),
    ];
    match body {
        Body::Text(text) => {
            pairs.push(("text", Value::text(text.text.clone())));
            // 没设过的样式不写，交给读的一方用主题默认值。
            if let Some(font) = &text.font {
                pairs.push(("font", Value::text(font.clone())));
            }
            if let Some(size) = text.size {
                pairs.push(("size", Value::Float(f64::from(size))));
            }
            if let Some(color) = text.color {
                pairs.push(("color", Value::text(color_text(color))));
            }
            if let Some(background) = text.background {
                pairs.push(("background", Value::text(color_text(background))));
            }
        }
        Body::Image(image) => {
            pairs.push(("media", Value::text(image.media.clone())));
            pairs.push(("name", Value::text(image.name.clone())));
            pairs.push(("fit", Value::text(fit_name(image.fit))));
        }
        Body::Media(media) => {
            pairs.push(("media", Value::text(media.media.clone())));
            pairs.push(("name", Value::text(media.name.clone())));
            pairs.push((
                "bytes",
                Value::Integer(i64::try_from(media.bytes).unwrap_or(i64::MAX)),
            ));
        }
        Body::Table(table) => {
            pairs.push(("rows", Value::Integer(i64::from(table.rows))));
            pairs.push(("cols", Value::Integer(i64::from(table.cols))));
            pairs.push((
                "cells",
                Value::Array(
                    table
                        .cells
                        .iter()
                        .map(|cell| Value::text(cell.clone()))
                        .collect(),
                ),
            ));
        }
        Body::Shape(shape) => {
            pairs.push(("shape", Value::text(shape.shape.name())));
            if let Some(fill) = shape.fill {
                pairs.push(("fill", Value::text(color_text(fill))));
            }
            if let Some(stroke) = shape.stroke {
                pairs.push(("stroke", Value::text(color_text(stroke))));
            }
            pairs.push(("stroke_width", Value::Float(f64::from(shape.stroke_width))));
        }
        Body::Highlight(highlight) => {
            pairs.push(("color", Value::text(color_text(highlight.color))));
            pairs.push(("opacity", Value::Float(f64::from(highlight.opacity))));
        }
    }
    Value::map(pairs)
}

pub(crate) fn body_from(value: &Value, kind: BlockKind) -> Result<Body, Error> {
    format_of(value)?;
    let found = value
        .get("block_type")
        .and_then(Value::as_integer)
        .ok_or(Error::MissingField("block_type"))?;
    if BlockKind::from_number(found) != Some(kind) {
        return Err(Error::WrongType("block_type"));
    }
    match kind {
        BlockKind::Text => text_from(value),
        BlockKind::Image => image_from(value),
        BlockKind::Video | BlockKind::Audio => media_from(value),
        BlockKind::Table => table_from(value),
        BlockKind::Shape => shape_from(value),
        BlockKind::Highlight => highlight_from(value),
    }
}

/// 这一块用的是哪份媒体。没有媒体的块给 None。
pub(crate) fn body_media(body: &Body) -> Option<&str> {
    match body {
        Body::Image(image) if !image.media.is_empty() => Some(&image.media),
        Body::Media(media) if !media.media.is_empty() => Some(&media.media),
        _ => None,
    }
}

pub(crate) const fn fit_name(fit: ImageFit) -> &'static str {
    match fit {
        ImageFit::Cover => "cover",
        ImageFit::Contain => "contain",
        ImageFit::Fill => "fill",
    }
}

fn fit_from_name(name: &str) -> ImageFit {
    match name {
        "contain" => ImageFit::Contain,
        "fill" => ImageFit::Fill,
        _ => ImageFit::Cover,
    }
}

fn text_from(value: &Value) -> Result<Body, Error> {
    let text = value
        .get("text")
        .and_then(Value::as_text)
        .ok_or(Error::MissingField("text"))?;
    Ok(Body::Text(TextBlock {
        text: text.to_owned(),
        font: value
            .get("font")
            .and_then(Value::as_text)
            .map(str::to_owned),
        size: value
            .get("size")
            .and_then(Value::as_float)
            .map(|size| size as f32),
        color: value
            .get("color")
            .and_then(Value::as_text)
            .and_then(parse_color),
        background: value
            .get("background")
            .and_then(Value::as_text)
            .and_then(parse_color),
    }))
}

fn image_from(value: &Value) -> Result<Body, Error> {
    let media = value
        .get("media")
        .and_then(Value::as_text)
        .ok_or(Error::MissingField("media"))?;
    Ok(Body::Image(ImageBlock {
        media: media.to_owned(),
        name: value
            .get("name")
            .and_then(Value::as_text)
            .unwrap_or_default()
            .to_owned(),
        fit: value
            .get("fit")
            .and_then(Value::as_text)
            .map_or(ImageFit::Cover, fit_from_name),
    }))
}

fn media_from(value: &Value) -> Result<Body, Error> {
    let media = value
        .get("media")
        .and_then(Value::as_text)
        .ok_or(Error::MissingField("media"))?;
    Ok(Body::Media(MediaBlock {
        media: media.to_owned(),
        name: value
            .get("name")
            .and_then(Value::as_text)
            .unwrap_or_default()
            .to_owned(),
        bytes: value
            .get("bytes")
            .and_then(Value::as_integer)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .unwrap_or(0),
    }))
}

fn table_from(value: &Value) -> Result<Body, Error> {
    let rows = dimension(value, "rows")?;
    let cols = dimension(value, "cols")?;
    let mut table = TableBlock::new(rows, cols);
    if let Some(cells) = value.get("cells").and_then(Value::as_array) {
        for (index, cell) in cells.iter().enumerate() {
            if index >= table.cells.len() {
                break;
            }
            if let Some(cell) = cell.as_text() {
                table.cells[index] = cell.to_owned();
            }
        }
    }
    Ok(Body::Table(table))
}

fn shape_from(value: &Value) -> Result<Body, Error> {
    let shape = value
        .get("shape")
        .and_then(Value::as_text)
        .and_then(ShapeKind::from_name)
        .ok_or(Error::WrongType("shape"))?;
    let stroke_width = value
        .get("stroke_width")
        .and_then(Value::as_float)
        .map_or(2.0, |width| width as f32);
    Ok(Body::Shape(ShapeBlock {
        shape,
        fill: value
            .get("fill")
            .and_then(Value::as_text)
            .and_then(parse_color),
        stroke: value
            .get("stroke")
            .and_then(Value::as_text)
            .and_then(parse_color),
        stroke_width: if stroke_width.is_finite() && stroke_width >= 0.0 {
            stroke_width
        } else {
            2.0
        },
    }))
}

fn highlight_from(value: &Value) -> Result<Body, Error> {
    let fallback = HighlightBlock::default();
    let color = value
        .get("color")
        .and_then(Value::as_text)
        .and_then(parse_color)
        .unwrap_or(fallback.color);
    let opacity = value
        .get("opacity")
        .and_then(Value::as_float)
        .filter(|opacity| opacity.is_finite())
        .map_or(fallback.opacity, |opacity| opacity as f32);
    Ok(Body::Highlight(HighlightBlock {
        color,
        opacity: opacity.clamp(0.0, 1.0),
    }))
}

fn format_of(value: &Value) -> Result<i64, Error> {
    let found = value
        .get("format")
        .and_then(Value::as_integer)
        .ok_or(Error::MissingField("format"))?;
    if found > FORMAT {
        return Err(Error::NewerFormat {
            found,
            supported: FORMAT,
        });
    }
    Ok(found)
}

/// 颜色写进文件是 #rrggbb；读不出来就当没设过，不猜。
fn parse_color(text: &str) -> Option<[u8; 3]> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let component = |range: core::ops::Range<usize>| u8::from_str_radix(&hex[range], 16).ok();
    Some([component(0..2)?, component(2..4)?, component(4..6)?])
}

fn color_text([red, green, blue]: [u8; 3]) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

fn integer_of(id: u64) -> i64 {
    i64::try_from(id).unwrap_or(i64::MAX)
}

/// 表的大小。0 或负数当坏数据，整块跳过。
fn dimension(value: &Value, key: &'static str) -> Result<u16, Error> {
    let number = value
        .get(key)
        .and_then(Value::as_integer)
        .ok_or(Error::MissingField(key))?;
    match u16::try_from(number) {
        Ok(size) if size > 0 => Ok(size),
        _ => Err(Error::WrongType(key)),
    }
}

fn number(value: &Value, key: &'static str) -> Option<f32> {
    value.get(key).and_then(Value::as_float).map(|n| n as f32)
}
