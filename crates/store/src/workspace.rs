//! 工作区：一个目录，里面放画板文件、块文件与媒体目录。
//!
//! 布局与字段名按 docs/design/format.md：画板一张一个文件，私有块按编号单独存，
//! 目录项用 block_ref 指向块（工作区内是相对路径），媒体以后按内容哈希存。
//!
//! 写盘顺序是先块后画板。中途失败最多留下没人引用的块文件，不会留下指向空处的
//! 目录项；被删块留下的孤儿文件由以后的回收步骤处理，这里不猜哪些能删。

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

use notecevia_core::board::{BlockKind, Board, Document, Entry, Geometry, TextBlock};
use thiserror::Error;

use crate::cbor::{self, CborError, Value};

/// 这一版写的格式号。读到更大的号就停下，让用户升级程序。
const FORMAT: i64 = 1;
const BOARDS_DIR: &str = "boards";
const BLOCKS_DIR: &str = "blocks";

#[derive(Debug, Error)]
pub enum Error {
    #[error("读写工作区失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("文件内容读不懂：{0}")]
    Cbor(#[from] CborError),
    #[error("画板文件缺少字段：{0}")]
    MissingField(&'static str),
    #[error("画板文件里 {0} 的值不对")]
    WrongType(&'static str),
    #[error("这个文件由更高版本的程序写入（格式 {found}，本程序支持 {supported}），请升级程序")]
    NewerFormat { found: i64, supported: i64 },
}

/// 一个工作区目录。
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// 打开或建立工作区。目录不存在就建，已有的内容不动。
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, Error> {
        let workspace = Self { root: root.into() };
        for dir in [BOARDS_DIR, BLOCKS_DIR, "media"] {
            fs::create_dir_all(workspace.root.join(dir))?;
        }
        Ok(workspace)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 最近的一张画板：编号最大的那个。没有画板时返回 None，第一次启动就是这种情况。
    pub fn load_latest(&self) -> Result<Option<(u64, Document)>, Error> {
        let mut latest = None;
        for entry in fs::read_dir(self.root.join(BOARDS_DIR))? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("board") {
                continue;
            }
            let Some(id) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.parse::<u64>().ok())
            else {
                continue;
            };
            let better = match &latest {
                Some((best, _)) => id > *best,
                None => true,
            };
            if better {
                latest = Some((id, path));
            }
        }
        let Some((id, _)) = latest else {
            return Ok(None);
        };
        Ok(Some((id, self.load(id)?)))
    }

    /// 下一个可用的画板编号。删过的不复用，免得旧文件被新内容顶替。
    pub fn next_board(&self) -> Result<u64, Error> {
        let mut next = 1;
        for entry in fs::read_dir(self.root.join(BOARDS_DIR))? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("board") {
                continue;
            }
            if let Some(id) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.parse::<u64>().ok())
            {
                next = next.max(id + 1);
            }
        }
        Ok(next)
    }

    /// 读一张画板与它引用的正文。画板文件不存在时给一张空画板。
    ///
    /// 单个块文件坏了只丢那一段正文，位置留着，用户能重新写。画板文件本身坏了才是
    /// 真错误——那种情况宁可报错让人看见，也不要拿空画板覆盖掉。
    pub fn load(&self, board_id: u64) -> Result<Document, Error> {
        let bytes = match fs::read(self.board_path(board_id)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Document::new("画板"));
            }
            Err(error) => return Err(error.into()),
        };

        let board = board_from(&cbor::decode(&bytes)?)?;
        let mut texts = BTreeMap::new();
        for entry in &board.entries {
            let reference = self.block_ref(board_id, entry.id);
            let Some(path) = self.resolve(&reference) else {
                continue;
            };
            let Ok(bytes) = fs::read(path) else {
                continue;
            };
            let Ok(value) = cbor::decode(&bytes) else {
                continue;
            };
            if let Ok(text) = text_from(&value) {
                texts.insert(entry.id, text);
            }
        }
        Ok(Document { board, texts })
    }

    /// 存盘。先写块再写画板，每个文件都是先写临时文件再改名。
    pub fn save(&self, board_id: u64, document: &Document) -> Result<(), Error> {
        for (id, text) in &document.texts {
            let value = text_value(*id, text);
            let path = self.root.join(self.block_ref(board_id, *id));
            write_atomic(&path, &cbor::encode(&value))?;
        }
        let value = self.board_value(board_id, &document.board);
        write_atomic(&self.board_path(board_id), &cbor::encode(&value))?;
        Ok(())
    }

    fn board_path(&self, board_id: u64) -> PathBuf {
        self.root.join(BOARDS_DIR).join(format!("{board_id}.board"))
    }

    /// 目录项里记的块地址：工作区内，正斜杠，跨平台一致。
    ///
    /// 块编号只在画板内唯一，所以文件名带上画板号，多张画板的块不会撞车。
    fn block_ref(&self, board_id: u64, id: u64) -> String {
        format!("{BLOCKS_DIR}/{board_id}-{id}.block")
    }

    /// 把画板里记的地址变成本机路径。
    ///
    /// 地址来自文件，不能直接当路径用：绝对路径和 .. 一律拒绝，否则一个改过的
    /// 画板文件就能让程序读写工作区之外的地方。
    fn resolve(&self, reference: &str) -> Option<PathBuf> {
        let path = Path::new(reference);
        if path.is_absolute() {
            return None;
        }
        if path.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return None;
        }
        Some(self.root.join(path))
    }

    fn board_value(&self, board_id: u64, board: &Board) -> Value {
        Value::map(vec![
            ("format", Value::Integer(FORMAT)),
            ("kind", Value::text("board")),
            ("title", Value::text(board.title.clone())),
            (
                "entries",
                Value::Array(
                    board
                        .entries
                        .iter()
                        .map(|entry| self.entry_value(board_id, entry))
                        .collect(),
                ),
            ),
        ])
    }

    fn entry_value(&self, board_id: u64, entry: &Entry) -> Value {
        Value::map(vec![
            ("id", Value::Integer(entry.id as i64)),
            ("block_type", Value::Integer(entry.kind.number())),
            ("block_ref", Value::text(self.block_ref(board_id, entry.id))),
            ("x", Value::Float(f64::from(entry.geometry.x))),
            ("y", Value::Float(f64::from(entry.geometry.y))),
            ("w", Value::Float(f64::from(entry.geometry.width))),
            ("h", Value::Float(f64::from(entry.geometry.height))),
            ("rotation", Value::Float(f64::from(entry.geometry.rotation))),
            ("z_index", Value::Integer(i64::from(entry.z))),
        ])
    }
}

fn text_value(id: u64, block: &TextBlock) -> Value {
    let mut pairs = vec![
        ("format", Value::Integer(FORMAT)),
        ("block_type", Value::Integer(BlockKind::Text.number())),
        ("id", Value::Integer(id as i64)),
        ("text", Value::text(block.text.clone())),
    ];
    // 没设过的样式不写，交给读的一方用主题默认值。
    if let Some(font) = &block.font {
        pairs.push(("font", Value::text(font.clone())));
    }
    if let Some(size) = block.size {
        pairs.push(("size", Value::Float(f64::from(size))));
    }
    if let Some([r, g, b]) = block.color {
        pairs.push(("color", Value::text(format!("#{r:02x}{g:02x}{b:02x}"))));
    }
    if let Some([r, g, b]) = block.background {
        pairs.push(("background", Value::text(format!("#{r:02x}{g:02x}{b:02x}"))));
    }
    Value::map(pairs)
}

/// 先写临时文件、落盘后再改名。改名是同目录的原子操作，
/// 所以断电最多丢掉这一次的修改，不会留下半个文件。
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let temp = path.with_extension("tmp");
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temp, path)?;
    Ok(())
}

fn format_of(value: &Value) -> Result<i64, Error> {
    let found = integer(value, "format")?;
    if found > FORMAT {
        return Err(Error::NewerFormat {
            found,
            supported: FORMAT,
        });
    }
    Ok(found)
}

fn board_from(value: &Value) -> Result<Board, Error> {
    format_of(value)?;
    let title = value
        .get("title")
        .and_then(Value::as_text)
        .ok_or(Error::MissingField("title"))?;
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .ok_or(Error::MissingField("entries"))?;

    let mut board = Board::new(title);
    for value in entries {
        // 单条目坏了只丢这一条，其余照读：不认识的块类型、坏掉的坐标都跳过。
        let id = integer(value, "id")?;
        let Ok(id) = u64::try_from(id) else {
            continue;
        };
        let kind = integer(value, "block_type")?;
        let Some(kind) = BlockKind::from_number(kind) else {
            continue;
        };
        let rotation = value
            .get("rotation")
            .and_then(Value::as_float)
            .unwrap_or(0.0) as f32;
        let geometry = Geometry {
            rotation,
            ..Geometry::new(
                float(value, "x")?,
                float(value, "y")?,
                float(value, "w")?,
                float(value, "h")?,
            )
        };
        if !geometry.is_valid() {
            continue;
        }
        let z = integer(value, "z_index")?;
        let Ok(z) = i32::try_from(z) else {
            continue;
        };
        board.entries.push(Entry {
            id,
            kind,
            geometry,
            z,
        });
    }
    Ok(board)
}

fn text_from(value: &Value) -> Result<TextBlock, Error> {
    format_of(value)?;
    let block_type = integer(value, "block_type")?;
    if BlockKind::from_number(block_type) != Some(BlockKind::Text) {
        return Err(Error::WrongType("block_type"));
    }
    let text = value
        .get("text")
        .and_then(Value::as_text)
        .ok_or(Error::MissingField("text"))?;
    let font = value
        .get("font")
        .and_then(Value::as_text)
        .map(str::to_owned);
    let size = value
        .get("size")
        .and_then(Value::as_float)
        .map(|size| size as f32);
    let color = value
        .get("color")
        .and_then(Value::as_text)
        .and_then(parse_color);
    let background = value
        .get("background")
        .and_then(Value::as_text)
        .and_then(parse_color);
    Ok(TextBlock {
        text: text.to_owned(),
        font,
        size,
        color,
        background,
    })
}

/// 颜色写成 #rrggbb。读不出来就当没设过，不猜。
fn parse_color(text: &str) -> Option<[u8; 3]> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let component = |range: core::ops::Range<usize>| u8::from_str_radix(&hex[range], 16).ok();
    Some([component(0..2)?, component(2..4)?, component(4..6)?])
}

fn integer(value: &Value, key: &'static str) -> Result<i64, Error> {
    value
        .get(key)
        .and_then(Value::as_integer)
        .ok_or(Error::MissingField(key))
}

fn float(value: &Value, key: &'static str) -> Result<f32, Error> {
    let number = value
        .get(key)
        .and_then(Value::as_float)
        .ok_or(Error::MissingField(key))?;
    Ok(number as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("notecevia-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        path
    }

    fn sample() -> Document {
        let mut document = Document::new("测试画板");
        let id = document.add_text(Geometry::new(12.0, 34.0, 320.0, 140.0));
        document.text_mut(id).text = "第一段中文".to_owned();
        document
    }

    #[test]
    fn save_then_load_gives_back_the_same_document() {
        let root = temp_root("roundtrip");
        let workspace = Workspace::open(&root).expect("open");
        let document = sample();
        workspace.save(1, &document).expect("save");
        assert_eq!(workspace.load(1).expect("load"), document);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn font_family_survives_a_roundtrip() {
        let root = temp_root("font");
        let workspace = Workspace::open(&root).expect("open");
        let mut document = sample();
        document.texts.get_mut(&1).expect("block").font = Some("SimSun".to_owned());
        workspace.save(1, &document).expect("save");
        assert_eq!(workspace.load(1).expect("load"), document);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn background_colour_survives_a_roundtrip() {
        let root = temp_root("background");
        let workspace = Workspace::open(&root).expect("open");
        let mut document = sample();
        document.texts.get_mut(&1).expect("block").background = Some([240, 230, 140]);
        workspace.save(1, &document).expect("save");
        assert_eq!(workspace.load(1).expect("load"), document);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rotation_survives_a_roundtrip() {
        let root = temp_root("rotation");
        let workspace = Workspace::open(&root).expect("open");
        let mut document = sample();
        document.board.entries[0].geometry.rotation = 0.75;
        workspace.save(1, &document).expect("save");
        assert_eq!(workspace.load(1).expect("load"), document);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn text_style_survives_a_roundtrip() {
        let root = temp_root("style");
        let workspace = Workspace::open(&root).expect("open");
        let mut document = sample();
        document.texts.get_mut(&1).expect("block").size = Some(26.0);
        document.texts.get_mut(&1).expect("block").color = Some([220, 90, 80]);
        workspace.save(1, &document).expect("save");
        assert_eq!(workspace.load(1).expect("load"), document);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn boards_are_numbered_and_the_latest_is_loaded() {
        let root = temp_root("boards");
        let workspace = Workspace::open(&root).expect("open");
        assert_eq!(workspace.load_latest().expect("load"), None);
        assert_eq!(workspace.next_board().expect("next"), 1);

        workspace.save(1, &sample()).expect("save 1");
        let mut second = Document::new("第二张");
        second.add_text(Geometry::new(0.0, 0.0, 100.0, 40.0));
        workspace.save(2, &second).expect("save 2");

        assert_eq!(workspace.next_board().expect("next"), 3);
        let (id, loaded) = workspace.load_latest().expect("latest").expect("some");
        assert_eq!(id, 2);
        assert_eq!(loaded, second);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn first_load_is_an_empty_board() {
        let root = temp_root("empty");
        let workspace = Workspace::open(&root).expect("open");
        let document = workspace.load(1).expect("load");
        assert!(document.board.entries.is_empty());
        assert!(document.texts.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn saving_twice_keeps_the_newer_content() {
        let root = temp_root("rewrite");
        let workspace = Workspace::open(&root).expect("open");
        workspace.save(1, &sample()).expect("save");
        let mut second = sample();
        second.texts.get_mut(&1).expect("block").text = "改过的内容".to_owned();
        workspace.save(1, &second).expect("save again");
        assert_eq!(workspace.load(1).expect("load"), second);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_block_file_keeps_the_position() {
        let root = temp_root("orphan");
        let workspace = Workspace::open(&root).expect("open");
        workspace.save(1, &sample()).expect("save");
        fs::remove_file(root.join("blocks").join("1-1.block")).expect("remove");
        let document = workspace.load(1).expect("load");
        assert_eq!(document.board.entries.len(), 1);
        assert!(document.texts.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn newer_format_is_refused() {
        let root = temp_root("newer");
        let workspace = Workspace::open(&root).expect("open");
        let value = Value::map(vec![
            ("format", Value::Integer(FORMAT + 1)),
            ("title", Value::text("未来的画板")),
            ("entries", Value::Array(Vec::new())),
        ]);
        fs::write(root.join("boards").join("1.board"), cbor::encode(&value)).expect("write");
        assert!(matches!(workspace.load(1), Err(Error::NewerFormat { .. })));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn broken_block_file_is_skipped() {
        let root = temp_root("broken");
        let workspace = Workspace::open(&root).expect("open");
        workspace.save(1, &sample()).expect("save");
        fs::write(root.join("blocks").join("1-1.block"), b"not cbor at all").expect("write");
        let document = workspace.load(1).expect("load");
        assert_eq!(document.board.entries.len(), 1);
        assert!(document.texts.is_empty());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn addresses_outside_the_workspace_are_refused() {
        let workspace = Workspace::open(temp_root("escape")).expect("open");
        assert!(workspace.resolve("blocks/1.block").is_some());
        assert!(workspace.resolve("../../secret").is_none());
        assert!(workspace.resolve("C:/Windows/system32").is_none());
        assert!(workspace.resolve("/etc/passwd").is_none());
    }
}
