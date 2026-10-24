//! 画板与块的数据形状。只描述数据，不碰文件、不碰时钟。

use std::collections::BTreeMap;

/// 一块内容的种类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Text,
}

impl BlockKind {
    /// 块类型编号。编号写进文件，改动等于改格式，要带迁移。
    pub const fn number(self) -> i64 {
        match self {
            Self::Text => 1,
        }
    }

    /// 从盘上的编号读回来。认不出的编号由调用方跳过，不猜。
    pub fn from_number(number: i64) -> Option<Self> {
        match number {
            1 => Some(Self::Text),
            _ => None,
        }
    }
}

/// 世界坐标里的矩形。缩放只影响显示，不改这里。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// 绕块中心顺时针旋转的弧度。
    pub rotation: f32,
}

impl Geometry {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            rotation: 0.0,
        }
    }

    pub fn size(self) -> (f32, f32) {
        (self.width, self.height)
    }

    /// 块中心。旋转绕这里转。
    pub fn center(self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// 宽高不许是负的或非有限值，读盘时用它兜底。
    pub fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.rotation.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
    }
}

/// 画板里的一条目录项：位置、层次、指向哪一块。
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// 目录项标识，画板内唯一且不再复用。块的正文按这个号单独存一个文件。
    pub id: u64,
    pub kind: BlockKind,
    pub geometry: Geometry,
    pub z: i32,
}

/// 文字块的正文与样式。
///
/// size 与 color 都是可空的：空表示跟随界面主题，这样浅色和深色都好看，
/// 用户明确选过的颜色才写死。将来加粗体、行内样式，从这里长一个 span 表出来。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextBlock {
    pub text: String,
    pub size: Option<f32>,
    pub color: Option<[u8; 3]>,
    pub background: Option<[u8; 3]>,
}

/// 一张画板：标题加目录。正文不在目录里，按需读取。
#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    pub title: String,
    pub entries: Vec<Entry>,
}

impl Board {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            entries: Vec::new(),
        }
    }

    /// 下一个没被占用的编号。删掉的号不复用，避免旧文件顶替新内容。
    pub fn next_id(&self) -> u64 {
        self.entries.iter().map(|entry| entry.id).max().unwrap_or(0) + 1
    }
}

/// 一张画板加它引用的正文。存盘、开画板都以这个整体为单位。
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub board: Board,
    pub texts: BTreeMap<u64, TextBlock>,
}

impl Document {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            board: Board::new(title),
            texts: BTreeMap::new(),
        }
    }

    /// 加一块文字块，返回它的编号。
    pub fn add_text(&mut self, geometry: Geometry) -> u64 {
        let id = self.board.next_id();
        self.board.entries.push(Entry {
            id,
            kind: BlockKind::Text,
            geometry,
            z: 0,
        });
        self.texts.insert(id, TextBlock::default());
        id
    }

    /// 读一块的正文。没有这块时给空字符串，界面不用到处判空。
    pub fn text_of(&self, id: u64) -> &str {
        self.texts.get(&id).map_or("", |text| text.text.as_str())
    }

    pub fn text_mut(&mut self, id: u64) -> &mut TextBlock {
        self.texts.entry(id).or_default()
    }

    /// 删掉一块。盘上留下的孤儿文件由回收步骤处理，这里只改内存。
    pub fn remove_block(&mut self, id: u64) {
        self.board.entries.retain(|entry| entry.id != id);
        self.texts.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_roundtrips_through_its_number() {
        assert_eq!(
            BlockKind::from_number(BlockKind::Text.number()),
            Some(BlockKind::Text)
        );
        assert_eq!(BlockKind::from_number(99), None);
    }

    #[test]
    fn next_id_does_not_reuse_deleted_numbers() {
        let mut board = Board::new("画板");
        assert_eq!(board.next_id(), 1);
        board.entries.push(Entry {
            id: 3,
            kind: BlockKind::Text,
            geometry: Geometry::new(0.0, 0.0, 10.0, 10.0),
            z: 0,
        });
        assert_eq!(board.next_id(), 4);
    }

    #[test]
    fn adding_text_blocks_numbers_them_upward() {
        let mut document = Document::new("画板");
        let first = document.add_text(Geometry::new(0.0, 0.0, 10.0, 10.0));
        let second = document.add_text(Geometry::new(0.0, 20.0, 10.0, 10.0));
        assert_eq!((first, second), (1, 2));
        assert_eq!(document.text_of(first), "");
        assert_eq!(document.board.entries.len(), 2);
    }

    #[test]
    fn removing_a_block_removes_its_text_too() {
        let mut document = Document::new("画板");
        let id = document.add_text(Geometry::new(0.0, 0.0, 10.0, 10.0));
        document.text_mut(id).text = "要删掉的".to_owned();
        document.remove_block(id);
        assert!(document.board.entries.is_empty());
        assert!(document.texts.is_empty());
    }

    #[test]
    fn zero_area_geometry_is_invalid() {
        assert!(!Geometry::new(0.0, 0.0, 0.0, 10.0).is_valid());
        assert!(!Geometry::new(0.0, 0.0, -1.0, 10.0).is_valid());
        assert!(Geometry::new(0.0, 0.0, 1.0, 10.0).is_valid());
    }
}
