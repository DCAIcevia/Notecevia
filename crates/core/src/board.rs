//! 画板与块的数据形状。只描述数据，不碰文件、不碰时钟。

use std::collections::BTreeMap;

/// 一块内容的种类。编号写进文件，改动等于改格式，要带迁移。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockKind {
    /// 文本：私有内容，按块编号存。
    Text,
    /// 图片：共享内容，按内容哈希进媒体池。
    Image,
    /// 视频：共享内容，按内容哈希进媒体池。
    Video,
    /// 音频：共享内容，按内容哈希进媒体池。
    Audio,
    /// 表格：私有内容，单元格内嵌纯文本。
    Table,
    /// 形状：私有内容，矩形、椭圆、线。
    Shape,
    /// 高亮：私有内容，一块带透明度的色块。
    Highlight,
}

impl BlockKind {
    /// 全部种类，界面新建菜单按这个顺序排。
    pub const ALL: [Self; 7] = [
        Self::Text,
        Self::Image,
        Self::Video,
        Self::Audio,
        Self::Table,
        Self::Shape,
        Self::Highlight,
    ];

    pub const fn number(self) -> i64 {
        match self {
            Self::Text => 1,
            Self::Image => 2,
            Self::Video => 3,
            Self::Audio => 4,
            Self::Table => 5,
            Self::Shape => 6,
            Self::Highlight => 7,
        }
    }

    /// 从盘上的编号读回来。认不出的编号由调用方跳过，不猜。
    pub const fn from_number(number: i64) -> Option<Self> {
        match number {
            1 => Some(Self::Text),
            2 => Some(Self::Image),
            3 => Some(Self::Video),
            4 => Some(Self::Audio),
            5 => Some(Self::Table),
            6 => Some(Self::Shape),
            7 => Some(Self::Highlight),
            _ => None,
        }
    }

    /// 媒体块的内容在共享媒体池里，其余块自己存正文。
    pub const fn is_media(self) -> bool {
        matches!(self, Self::Image | Self::Video | Self::Audio)
    }

    /// 新建这个种类的块时给的初始正文。
    fn default_body(self) -> Body {
        match self {
            Self::Text => Body::Text(TextBlock::default()),
            Self::Image => Body::Image(ImageBlock::default()),
            Self::Video | Self::Audio => Body::Media(MediaBlock::default()),
            Self::Table => Body::Table(TableBlock::default()),
            Self::Shape => Body::Shape(ShapeBlock::default()),
            Self::Highlight => Body::Highlight(HighlightBlock::default()),
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
    /// 层级：大的画在上面。同层按目录顺序。
    pub z: i32,
    /// 锁住的块不能拖动、缩放、旋转，但仍可被选中查看。
    pub locked: bool,
    /// 隐藏的块不画、不响应点击，但仍留在文件里。
    pub visible: bool,
}

impl Entry {
    pub fn new(id: u64, kind: BlockKind, geometry: Geometry) -> Self {
        Self {
            id,
            kind,
            geometry,
            z: 0,
            locked: false,
            visible: true,
        }
    }
}

/// 文字块的正文与样式。
///
/// size 与 color 都是可空的：空表示跟随界面主题，这样浅色和深色都好看，
/// 用户明确选过的颜色才写死。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextBlock {
    pub text: String,
    /// 字体族名，省掉表示跟随主题默认字体。
    pub font: Option<String>,
    pub size: Option<f32>,
    pub color: Option<[u8; 3]>,
    pub background: Option<[u8; 3]>,
}

/// 图片在框里的填充方式。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageFit {
    /// 铺满框，超出部分裁掉。
    #[default]
    Cover,
    /// 整张图都放进来，留空处。
    Contain,
    /// 拉伸到框的形状。
    Fill,
}

/// 图片块：内容在媒体池里，这里只记哈希与显示方式。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageBlock {
    /// 媒体池里的文件名，内容哈希。
    pub media: String,
    /// 原文件名，界面上显示用。
    pub name: String,
    pub fit: ImageFit,
}

/// 视频与音频块。种类由目录项决定，这里只记共用的元数据。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaBlock {
    pub media: String,
    pub name: String,
    /// 原文件大小，界面显示用。
    pub bytes: u64,
}

/// 表格块：行列数固定，单元格是纯文本，行优先展开。
#[derive(Clone, Debug, PartialEq)]
pub struct TableBlock {
    pub rows: u16,
    pub cols: u16,
    pub cells: Vec<String>,
}

impl Default for TableBlock {
    fn default() -> Self {
        Self::new(3, 3)
    }
}

impl TableBlock {
    pub fn new(rows: u16, cols: u16) -> Self {
        let rows = rows.max(1);
        let cols = cols.max(1);
        let count = usize::from(rows) * usize::from(cols);
        Self {
            rows,
            cols,
            cells: vec![String::new(); count],
        }
    }

    /// 读出某格；越界给空串，界面不用到处判空。
    pub fn cell(&self, row: u16, col: u16) -> &str {
        if row >= self.rows || col >= self.cols {
            return "";
        }
        let index = usize::from(row) * usize::from(self.cols) + usize::from(col);
        self.cells.get(index).map_or("", String::as_str)
    }

    /// 写某格；行列数变了就不写，避免把表撑坏。
    pub fn set_cell(&mut self, row: u16, col: u16, text: impl Into<String>) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        let index = usize::from(row) * usize::from(self.cols) + usize::from(col);
        if let Some(cell) = self.cells.get_mut(index) {
            *cell = text.into();
        }
    }

    /// 加一行或加一列，已有内容位置不变。
    pub fn add_row(&mut self) {
        self.rows = self.rows.saturating_add(1);
        self.cells
            .extend(std::iter::repeat_n(String::new(), usize::from(self.cols)));
    }

    pub fn add_col(&mut self) {
        let rows = self.rows;
        let cols = self.cols;
        let mut cells = Vec::with_capacity(usize::from(rows) * usize::from(cols + 1));
        for row in 0..rows {
            for col in 0..cols {
                let index = usize::from(row) * usize::from(cols) + usize::from(col);
                cells.push(self.cells.get(index).cloned().unwrap_or_default());
            }
            cells.push(String::new());
        }
        self.cols = cols + 1;
        self.cells = cells;
    }
}

/// 形状的种类。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShapeKind {
    #[default]
    Rect,
    Ellipse,
    Line,
}

impl ShapeKind {
    pub const ALL: [Self; 3] = [Self::Rect, Self::Ellipse, Self::Line];

    /// 写进文件的短名。
    pub const fn name(self) -> &'static str {
        match self {
            Self::Rect => "rect",
            Self::Ellipse => "ellipse",
            Self::Line => "line",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "rect" => Some(Self::Rect),
            "ellipse" => Some(Self::Ellipse),
            "line" => Some(Self::Line),
            _ => None,
        }
    }
}

/// 形状块：几何参数、填充、描边。
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeBlock {
    pub shape: ShapeKind,
    pub fill: Option<[u8; 3]>,
    pub stroke: Option<[u8; 3]>,
    pub stroke_width: f32,
}

impl Default for ShapeBlock {
    fn default() -> Self {
        Self {
            shape: ShapeKind::default(),
            fill: Some([0x8a, 0x9b, 0xb4]),
            stroke: None,
            stroke_width: 2.0,
        }
    }
}

/// 高亮块：一块带透明度的色，垫在文字下面。
#[derive(Clone, Debug, PartialEq)]
pub struct HighlightBlock {
    pub color: [u8; 3],
    /// 0 到 1。
    pub opacity: f32,
}

impl Default for HighlightBlock {
    fn default() -> Self {
        Self {
            color: [0xff, 0xe0, 0x66],
            opacity: 0.55,
        }
    }
}

/// 一个块的正文。目录项与正文按块编号一对一。
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Text(TextBlock),
    Image(ImageBlock),
    Media(MediaBlock),
    Table(TableBlock),
    Shape(ShapeBlock),
    Highlight(HighlightBlock),
}

impl Body {
    pub const fn kind(&self) -> BlockKind {
        match self {
            Self::Text(_) => BlockKind::Text,
            Self::Image(_) => BlockKind::Image,
            Self::Media(_) => BlockKind::Audio,
            Self::Table(_) => BlockKind::Table,
            Self::Shape(_) => BlockKind::Shape,
            Self::Highlight(_) => BlockKind::Highlight,
        }
    }
}

/// 一张画板：标题、标签加目录。正文不在目录里，按需读取。
#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    pub title: String,
    /// 标签，去重后按登记顺序存。
    pub tags: Vec<String>,
    pub entries: Vec<Entry>,
}

impl Board {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            tags: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// 下一个没被占用的编号。删掉的号不复用，避免旧文件顶替新内容。
    pub fn next_id(&self) -> u64 {
        self.entries.iter().map(|entry| entry.id).max().unwrap_or(0) + 1
    }

    /// 加上一个标签；已经有的不重复加。
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let tag = tag.into();
        if tag.is_empty() || self.tags.contains(&tag) {
            return;
        }
        self.tags.push(tag);
    }
}

/// 一张画板加它引用的正文。存盘、开画板都以这个整体为单位。
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub board: Board,
    pub bodies: BTreeMap<u64, Body>,
}

impl Document {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            board: Board::new(title),
            bodies: BTreeMap::new(),
        }
    }

    pub fn next_id(&self) -> u64 {
        self.board.next_id()
    }

    /// 加一块，返回它的编号。正文给这个种类的默认值。
    pub fn add_block(&mut self, kind: BlockKind, geometry: Geometry) -> u64 {
        let id = self.board.next_id();
        self.board.entries.push(Entry::new(id, kind, geometry));
        self.bodies.insert(id, kind.default_body());
        id
    }

    /// 加一块文字块，返回它的编号。
    pub fn add_text(&mut self, geometry: Geometry) -> u64 {
        self.add_block(BlockKind::Text, geometry)
    }

    pub fn body(&self, id: u64) -> Option<&Body> {
        self.bodies.get(&id)
    }

    pub fn body_mut(&mut self, id: u64) -> Option<&mut Body> {
        self.bodies.get_mut(&id)
    }

    /// 读一块的正文。没有这块时给空字符串，界面不用到处判空。
    pub fn text_of(&self, id: u64) -> &str {
        match self.bodies.get(&id) {
            Some(Body::Text(text)) => text.text.as_str(),
            _ => "",
        }
    }

    pub fn text_mut(&mut self, id: u64) -> &mut TextBlock {
        if !matches!(self.bodies.get(&id), Some(Body::Text(_))) {
            self.bodies.insert(id, Body::Text(TextBlock::default()));
        }
        match self.bodies.get_mut(&id) {
            Some(Body::Text(text)) => text,
            // 上一段刚保证过这块是文本块；走到这里说明有人绕过方法换了正文。
            _ => panic!("文本块的正文被换成了别的种类"),
        }
    }

    pub fn entry(&self, id: u64) -> Option<&Entry> {
        self.board.entries.iter().find(|entry| entry.id == id)
    }

    pub fn entry_mut(&mut self, id: u64) -> Option<&mut Entry> {
        self.board.entries.iter_mut().find(|entry| entry.id == id)
    }

    /// 删掉一块，返回是否真的删了。盘上留下的孤儿文件由回收步骤处理。
    pub fn remove_block(&mut self, id: u64) -> bool {
        let before = self.board.entries.len();
        self.board.entries.retain(|entry| entry.id != id);
        self.bodies.remove(&id);
        self.board.entries.len() != before
    }

    /// 复制一块，错开一点放，返回新编号。
    pub fn duplicate_block(&mut self, id: u64) -> Option<u64> {
        let entry = self.entry(id)?.clone();
        let body = self.body(id)?.clone();
        let new_id = self.board.next_id();
        let z = self.max_z() + 1;
        let mut geometry = entry.geometry;
        geometry.x += 24.0;
        geometry.y += 24.0;
        self.board.entries.push(Entry {
            id: new_id,
            kind: entry.kind,
            geometry,
            z,
            locked: false,
            visible: true,
        });
        self.bodies.insert(new_id, body);
        Some(new_id)
    }

    pub fn max_z(&self) -> i32 {
        self.board
            .entries
            .iter()
            .map(|entry| entry.z)
            .max()
            .unwrap_or(0)
    }

    fn min_z(&self) -> i32 {
        self.board
            .entries
            .iter()
            .map(|entry| entry.z)
            .min()
            .unwrap_or(0)
    }

    /// 把选中的块放到最上面，选中的块之间保持原来的先后。
    pub fn bring_to_front(&mut self, ids: &[u64]) {
        let mut z = self.max_z();
        for id in ids {
            z += 1;
            if let Some(entry) = self.entry_mut(*id) {
                entry.z = z;
            }
        }
    }

    /// 把选中的块放到最下面。
    pub fn send_to_back(&mut self, ids: &[u64]) {
        let mut z = self.min_z();
        for id in ids {
            z -= 1;
            if let Some(entry) = self.entry_mut(*id) {
                entry.z = z;
            }
        }
    }

    pub fn set_locked(&mut self, ids: &[u64], locked: bool) {
        for id in ids {
            if let Some(entry) = self.entry_mut(*id) {
                entry.locked = locked;
            }
        }
    }

    pub fn set_visible(&mut self, ids: &[u64], visible: bool) {
        for id in ids {
            if let Some(entry) = self.entry_mut(*id) {
                entry.visible = visible;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_roundtrips_through_its_number() {
        for kind in BlockKind::ALL {
            assert_eq!(BlockKind::from_number(kind.number()), Some(kind));
        }
        assert_eq!(BlockKind::from_number(99), None);
    }

    #[test]
    fn next_id_does_not_reuse_deleted_numbers() {
        let mut board = Board::new("画板");
        assert_eq!(board.next_id(), 1);
        board.entries.push(Entry::new(
            3,
            BlockKind::Text,
            Geometry::new(0.0, 0.0, 10.0, 10.0),
        ));
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
        assert!(document.remove_block(id));
        assert_eq!(document.text_of(id), "");
        assert!(document.body(id).is_none());
        assert!(!document.remove_block(id));
    }

    #[test]
    fn every_kind_gets_a_matching_default_body() {
        let mut document = Document::new("画板");
        for kind in BlockKind::ALL {
            let id = document.add_block(kind, Geometry::new(0.0, 0.0, 10.0, 10.0));
            let body = document.body(id).expect("加块就有正文");
            if kind == BlockKind::Video || kind == BlockKind::Audio {
                assert_eq!(body.kind(), BlockKind::Audio);
            } else {
                assert_eq!(body.kind(), kind);
            }
        }
    }

    #[test]
    fn duplicate_offsets_and_keeps_content() {
        let mut document = Document::new("画板");
        let id = document.add_text(Geometry::new(10.0, 10.0, 30.0, 30.0));
        document.text_mut(id).text = "原件".to_owned();
        let copy = document.duplicate_block(id).expect("有原件就能复制");
        assert_ne!(copy, id);
        assert_eq!(document.text_of(copy), "原件");
        assert_eq!(document.entry(copy).unwrap().geometry.x, 34.0);
    }

    #[test]
    fn front_and_back_move_z_in_order() {
        let mut document = Document::new("画板");
        let first = document.add_text(Geometry::new(0.0, 0.0, 10.0, 10.0));
        let second = document.add_text(Geometry::new(0.0, 0.0, 10.0, 10.0));
        document.bring_to_front(&[first, second]);
        assert!(document.entry(second).unwrap().z > document.entry(first).unwrap().z);
        let smallest = document.min_z();
        document.send_to_back(&[second]);
        assert_eq!(document.entry(second).unwrap().z, smallest - 1);
    }

    #[test]
    fn table_cells_survive_size_changes() {
        let mut table = TableBlock::new(2, 2);
        table.set_cell(0, 0, "甲");
        table.set_cell(1, 1, "乙");
        table.add_row();
        table.add_col();
        assert_eq!(table.cell(0, 0), "甲");
        assert_eq!(table.cell(1, 1), "乙");
        assert_eq!(table.cell(2, 2), "");
        assert_eq!(table.cells.len(), 9);
    }

    #[test]
    fn tags_are_unique() {
        let mut board = Board::new("画板");
        board.add_tag("工作");
        board.add_tag("工作");
        board.add_tag("");
        board.add_tag("灵感");
        assert_eq!(board.tags, vec!["工作".to_owned(), "灵感".to_owned()]);
    }
}
