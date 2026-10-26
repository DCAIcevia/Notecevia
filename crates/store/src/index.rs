//! 首页索引：列表、搜索与“打开最近画板”都只读这一份，不再逐板读文件。
//!
//! 索引是缓存，不是数据源：画板文件才是唯一事实。索引缺失、格式号不符、
//! 画板集合对不上（多、少、文件长度或修改时间变了）时整份重建；重建就是逐板读一遍。
//! 索引写不进去也不影响内容——本次先按内存里的用，下次打开再试。
//!
//! 一致性在每次打开工作区后第一次用到时校验一次；运行期间外部改动要重新打开才能发现。

use std::cell::Ref;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use notecevia_core::board::{Board, Body, Document};

use crate::cbor::{self, Value};
use crate::error::Error;
use crate::format::BoardOnDisk;
use crate::workspace::{Workspace, write_atomic};

/// 索引文件名，放在工作区根目录。删掉它没有副作用，下次用到会重建。
pub(crate) const INDEX_FILE: &str = "index.cbor";
/// 索引自己的格式号。字段或内容语义变了就加一，读到不认识的号当没有索引。
///
/// 2：可搜索片段加入表格单元格文字与媒体块文件名，旧索引必须重建。
const INDEX_FORMAT: i64 = 2;

/// 列表页的一行。正文不在这里，点开才读。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardInfo {
    pub id: u64,
    pub title: String,
    pub tags: Vec<String>,
    /// Unix 秒。
    pub updated_at: i64,
    pub entry_count: usize,
}

/// 一条搜索命中。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchHit {
    pub board_id: u64,
    pub title: String,
    /// 命中处前后各留一段，找不到正文时就给标题或标签。
    pub snippet: String,
}

/// 摘要里命中处前后各留多少字。
const SNIPPET_BEFORE: usize = 24;
const SNIPPET_AFTER: usize = 60;

/// 画板文件的一眼指纹：长度与修改时间。和索引里记的不一样就当索引过期。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileStamp {
    len: u64,
    modified: i64,
}

/// 索引里的一张画板。
pub(crate) struct IndexedBoard {
    pub title: String,
    pub tags: Vec<String>,
    pub updated_at: i64,
    pub entry_count: usize,
    pub has_thumbnail: bool,
    /// 可搜索片段，按目录顺序：文本块正文、表格单元格、媒体块文件名。
    pub texts: Vec<String>,
    /// 画板文件读不懂时留个占位：集合对得上，列表与搜索跳过它。
    pub readable: bool,
    /// 文件指纹；拿不到就当过期，下次打开重建。
    stamp: Option<FileStamp>,
}

impl IndexedBoard {
    fn broken(stamp: Option<FileStamp>) -> Self {
        Self {
            title: String::new(),
            tags: Vec::new(),
            updated_at: 0,
            entry_count: 0,
            has_thumbnail: false,
            texts: Vec::new(),
            readable: false,
            stamp,
        }
    }

    fn to_value(&self, id: u64) -> Value {
        let mut pairs = vec![
            ("id", Value::Integer(integer_of(id))),
            ("title", Value::text(self.title.clone())),
            (
                "tags",
                Value::Array(
                    self.tags
                        .iter()
                        .map(|tag| Value::text(tag.clone()))
                        .collect(),
                ),
            ),
            ("updated_at", Value::Integer(self.updated_at)),
            (
                "entry_count",
                Value::Integer(integer_of(self.entry_count as u64)),
            ),
            ("readable", Value::Bool(self.readable)),
            ("has_thumbnail", Value::Bool(self.has_thumbnail)),
            (
                "texts",
                Value::Array(
                    self.texts
                        .iter()
                        .map(|text| Value::text(text.clone()))
                        .collect(),
                ),
            ),
        ];
        if let Some(stamp) = self.stamp {
            pairs.push(("stamp_len", Value::Integer(integer_of(stamp.len))));
            pairs.push(("stamp_modified", Value::Integer(stamp.modified)));
        }
        Value::map(pairs)
    }

    fn from_value(value: &Value) -> Option<Self> {
        let stamp = match (
            value.get("stamp_len").and_then(Value::as_integer),
            value.get("stamp_modified").and_then(Value::as_integer),
        ) {
            (Some(len), Some(modified)) => Some(FileStamp {
                len: u64::try_from(len).ok()?,
                modified,
            }),
            _ => None,
        };
        Some(Self {
            title: value
                .get("title")
                .and_then(Value::as_text)
                .unwrap_or_default()
                .to_owned(),
            tags: texts_of(value, "tags"),
            updated_at: value
                .get("updated_at")
                .and_then(Value::as_integer)
                .unwrap_or(0),
            entry_count: value
                .get("entry_count")
                .and_then(Value::as_integer)
                .and_then(|count| usize::try_from(count).ok())
                .unwrap_or(0),
            has_thumbnail: value
                .get("has_thumbnail")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            texts: texts_of(value, "texts"),
            readable: value
                .get("readable")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            stamp,
        })
    }
}

/// 整份索引：画板编号 → 条目。
pub(crate) struct Index {
    pub boards: BTreeMap<u64, IndexedBoard>,
}

impl Index {
    fn to_value(&self) -> Value {
        Value::map(vec![
            ("format", Value::Integer(INDEX_FORMAT)),
            (
                "boards",
                Value::Array(
                    self.boards
                        .iter()
                        .map(|(id, board)| board.to_value(*id))
                        .collect(),
                ),
            ),
        ])
    }

    fn from_value(value: &Value) -> Option<Self> {
        if value.get("format").and_then(Value::as_integer) != Some(INDEX_FORMAT) {
            return None;
        }
        let boards = value.get("boards").and_then(Value::as_array)?;
        let mut map = BTreeMap::new();
        for board in boards {
            let id = board.get("id").and_then(Value::as_integer)?;
            let id = u64::try_from(id).ok()?;
            map.insert(id, IndexedBoard::from_value(board)?);
        }
        Some(Self { boards: map })
    }
}

impl Workspace {
    /// 全部画板，按修改时间从新到旧；时间相同按编号大的在前。
    ///
    /// 单个画板读不懂时跳过它：一个坏文件不该让首页整个打不开。文件本身还在。
    pub fn list_boards(&self) -> Result<Vec<BoardInfo>, Error> {
        let index = self.ensure_index()?;
        let mut boards: Vec<BoardInfo> = index
            .boards
            .iter()
            .filter(|(_, board)| board.readable)
            .map(|(&id, board)| BoardInfo {
                id,
                title: board.title.clone(),
                tags: board.tags.clone(),
                updated_at: board.updated_at,
                entry_count: board.entry_count,
            })
            .collect();
        boards.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(boards)
    }

    /// 在标题、标签和文本块正文里找。大小写不敏感；空查询给空结果。
    pub fn search(&self, query: &str) -> Result<Vec<SearchHit>, Error> {
        let needle: Vec<char> = query.trim().chars().collect();
        if needle.is_empty() {
            return Ok(Vec::new());
        }

        let index = self.ensure_index()?;
        let mut hits: Vec<(i64, u64, SearchHit)> = Vec::new();
        for (&id, board) in &index.boards {
            if !board.readable {
                continue;
            }
            let title = chars_of(&board.title);
            let mut snippet = find_chars(&title, &needle).map(|_| board.title.clone());
            if snippet.is_none() {
                snippet = board
                    .tags
                    .iter()
                    .find(|tag| find_chars(&chars_of(tag), &needle).is_some())
                    .map(|tag| format!("标签：{tag}"));
            }
            if snippet.is_none() {
                snippet = first_text_match(&board.texts, &needle);
            }
            if let Some(snippet) = snippet {
                hits.push((
                    board.updated_at,
                    id,
                    SearchHit {
                        board_id: id,
                        title: board.title.clone(),
                        snippet,
                    },
                ));
            }
        }
        // 与列表顺序一致：最近改的在前，同时间编号大的在前。
        hits.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
        Ok(hits.into_iter().map(|(_, _, hit)| hit).collect())
    }

    /// 强制重读 boards/、丢掉内存索引并重建，用于外部改了工作区后刷新首页（Ctrl+R）。
    ///
    /// 返回重建后的画板条数（按 boards/ 里的画板文件数算，读不懂的也算一条占位）。
    /// 只重写 index.cbor，不动画板、块与媒体文件；索引文件坏了也没关系，这里不读它。
    pub fn rebuild_index(&self) -> Result<usize, Error> {
        let index = self.build_index()?;
        let count = index.boards.len();
        // 先写盘再换内存：写不进去就报错，内存里仍是旧索引。
        self.write_index(&index)?;
        *self.index.borrow_mut() = Some(index);
        Ok(count)
    }

    /// 取索引；第一次用到时从磁盘读，缺失或过期就重建。
    pub(crate) fn ensure_index(&self) -> Result<Ref<'_, Index>, Error> {
        let loaded = self.index.borrow().is_some();
        if !loaded {
            let index = match self.read_index() {
                Some(index) if self.matches_boards(&index)? => index,
                _ => {
                    let built = self.build_index()?;
                    // 索引是缓存：写不进去也让本次读取继续，下次打开会再试。
                    let _ = self.write_index(&built);
                    built
                }
            };
            *self.index.borrow_mut() = Some(index);
        }
        // 上面刚把 Some 放进去；走到 Err 说明有人绕过这里改了缓存。
        match Ref::filter_map(self.index.borrow(), Option::as_ref) {
            Ok(index) => Ok(index),
            Err(_) => unreachable!("索引刚加载过，不可能是空的"),
        }
    }

    /// 保存成功后更新索引里的这一条，再整份原子重写索引文件。
    pub(crate) fn remember_board(
        &self,
        board_id: u64,
        document: &Document,
        updated_at: i64,
    ) -> Result<(), Error> {
        self.ensure_index()?;
        let entry = IndexedBoard {
            title: document.board.title.clone(),
            tags: document.board.tags.clone(),
            updated_at,
            entry_count: document.board.entries.len(),
            has_thumbnail: self.thumbnail_path(board_id).is_file(),
            texts: searchable_texts(&document.board, &document.bodies),
            readable: true,
            stamp: stamp_of_path(&self.board_path(board_id)),
        };
        self.store_entry(board_id, entry)
    }

    /// 画板文件刚被别处写完（比如容器导入）时，按盘上的内容补一条索引。
    pub(crate) fn remember_board_from_disk(&self, board_id: u64) -> Result<(), Error> {
        self.ensure_index()?;
        let stamp = stamp_of_path(&self.board_path(board_id));
        let entry = match self.read_board_lenient(board_id)? {
            Some(on_disk) => {
                let bodies = self.load_bodies(board_id, &on_disk);
                indexed_board(
                    &on_disk,
                    &bodies,
                    self.thumbnail_path(board_id).is_file(),
                    stamp,
                )
            }
            None => IndexedBoard::broken(stamp),
        };
        self.store_entry(board_id, entry)
    }

    fn store_entry(&self, board_id: u64, entry: IndexedBoard) -> Result<(), Error> {
        let value = {
            let mut guard = self.index.borrow_mut();
            let Some(index) = guard.as_mut() else {
                return Ok(());
            };
            index.boards.insert(board_id, entry);
            index.to_value()
        };
        self.write_index_value(&value)
    }

    /// 删掉画板后从索引里去掉这一条。
    pub(crate) fn forget_board(&self, board_id: u64) -> Result<(), Error> {
        self.ensure_index()?;
        let value = {
            let mut guard = self.index.borrow_mut();
            let Some(index) = guard.as_mut() else {
                return Ok(());
            };
            if index.boards.remove(&board_id).is_none() {
                return Ok(());
            }
            index.to_value()
        };
        self.write_index_value(&value)
    }

    /// 缩略图有变化时更新索引里的标记，避免下次打开前首页读不到。
    pub(crate) fn set_thumbnail(&self, board_id: u64, present: bool) -> Result<(), Error> {
        self.ensure_index()?;
        let value = {
            let mut guard = self.index.borrow_mut();
            let Some(index) = guard.as_mut() else {
                return Ok(());
            };
            let Some(board) = index.boards.get_mut(&board_id) else {
                return Ok(());
            };
            if board.has_thumbnail == present {
                return Ok(());
            }
            board.has_thumbnail = present;
            index.to_value()
        };
        self.write_index_value(&value)
    }

    /// 读索引文件。缺失、损坏、格式号不认识都当没有索引，交给重建。
    fn read_index(&self) -> Option<Index> {
        let bytes = fs::read(self.index_path()).ok()?;
        let value = cbor::decode(&bytes).ok()?;
        Index::from_value(&value)
    }

    /// 索引与 boards/ 里的画板集合一致吗：数量、编号、文件指纹都要对得上。
    fn matches_boards(&self, index: &Index) -> Result<bool, Error> {
        let ids = self.board_ids()?;
        if ids.len() != index.boards.len() {
            return Ok(false);
        }
        for id in ids {
            let Some(board) = index.boards.get(&id) else {
                return Ok(false);
            };
            let Some(expected) = board.stamp else {
                return Ok(false);
            };
            let Ok(metadata) = fs::metadata(self.board_path(id)) else {
                return Ok(false);
            };
            if stamp_of(&metadata) != Some(expected) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// 逐板读一遍，重建整份索引。
    fn build_index(&self) -> Result<Index, Error> {
        let mut boards = BTreeMap::new();
        for id in self.board_ids()? {
            let path = self.board_path(id);
            let stamp = stamp_of_path(&path);
            let board = match self.read_board_lenient(id)? {
                Some(on_disk) => {
                    let bodies = self.load_bodies(id, &on_disk);
                    indexed_board(&on_disk, &bodies, self.thumbnail_path(id).is_file(), stamp)
                }
                // 画板文件读不懂：留占位让集合对得上，列表与搜索跳过它。
                None => IndexedBoard::broken(stamp),
            };
            boards.insert(id, board);
        }
        Ok(Index { boards })
    }

    fn write_index(&self, index: &Index) -> Result<(), Error> {
        self.write_index_value(&index.to_value())
    }

    fn write_index_value(&self, value: &Value) -> Result<(), Error> {
        write_atomic(&self.index_path(), &cbor::encode(value))
    }
}

/// 从读出来的画板与正文拼一条索引项。
fn indexed_board(
    on_disk: &BoardOnDisk,
    bodies: &BTreeMap<u64, Body>,
    has_thumbnail: bool,
    stamp: Option<FileStamp>,
) -> IndexedBoard {
    IndexedBoard {
        title: on_disk.board.title.clone(),
        tags: on_disk.board.tags.clone(),
        updated_at: on_disk.updated_at,
        entry_count: on_disk.board.entries.len(),
        has_thumbnail,
        texts: searchable_texts(&on_disk.board, bodies),
        readable: true,
        stamp,
    }
}

/// 一张画板里能搜的文字，按目录顺序：文本块正文、表格单元格、媒体块文件名。
///
/// 表格按行优先把非空单元格用空格连成一段；媒体块（图片、视频、音频）收 name。
fn searchable_texts(board: &Board, bodies: &BTreeMap<u64, Body>) -> Vec<String> {
    let mut texts = Vec::new();
    for entry in &board.entries {
        match bodies.get(&entry.id) {
            Some(Body::Text(block)) if !block.text.is_empty() => texts.push(block.text.clone()),
            Some(Body::Table(block)) => {
                let joined = block
                    .cells
                    .iter()
                    .filter(|cell| !cell.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                if !joined.is_empty() {
                    texts.push(joined);
                }
            }
            Some(Body::Image(block)) if !block.name.is_empty() => texts.push(block.name.clone()),
            Some(Body::Media(block)) if !block.name.is_empty() => texts.push(block.name.clone()),
            _ => {}
        }
    }
    texts
}

fn stamp_of(metadata: &fs::Metadata) -> Option<FileStamp> {
    let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some(FileStamp {
        len: metadata.len(),
        modified: i64::try_from(modified.as_nanos()).ok()?,
    })
}

fn stamp_of_path(path: &Path) -> Option<FileStamp> {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| stamp_of(&metadata))
}

fn integer_of(number: u64) -> i64 {
    i64::try_from(number).unwrap_or(i64::MAX)
}

fn texts_of(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_text)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn chars_of(text: &str) -> Vec<char> {
    text.chars().collect()
}

/// 忽大小写比较两个字。ASCII 走快路径，其余按 Unicode 小写展开比。
fn eq_ignore_case(left: char, right: char) -> bool {
    if left.is_ascii() && right.is_ascii() {
        left.eq_ignore_ascii_case(&right)
    } else {
        left.to_lowercase().eq(right.to_lowercase())
    }
}

/// 按字符比较、忽大小写。中文没有大小写，这一步对它没有影响。
fn find_chars(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&start| {
        haystack[start..start + needle.len()]
            .iter()
            .zip(needle)
            .all(|(left, right)| eq_ignore_case(*left, *right))
    })
}

/// 命中处前后的摘要。换行压成空格，两端看不见的部分用省略号标出来。
fn snippet_chars(haystack: &[char], needle: &[char]) -> Option<String> {
    let found = find_chars(haystack, needle)?;
    let start = found.saturating_sub(SNIPPET_BEFORE);
    let end = (found + needle.len() + SNIPPET_AFTER).min(haystack.len());
    let excerpt: String = haystack[start..end].iter().copied().collect();
    let mut snippet = excerpt.split_whitespace().collect::<Vec<_>>().join(" ");
    if start > 0 {
        snippet.insert(0, '…');
    }
    if end < haystack.len() {
        snippet.push('…');
    }
    Some(snippet)
}

/// 第一段命中的正文。按目录顺序找，先找到先用。
fn first_text_match(texts: &[String], needle: &[char]) -> Option<String> {
    for text in texts {
        if let Some(snippet) = snippet_chars(&chars_of(text), needle) {
            return Some(snippet);
        }
    }
    None
}
