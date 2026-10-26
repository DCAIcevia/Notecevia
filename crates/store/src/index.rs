//! 首页要的两件事：画板列表与全文搜索。只读画板文件与文本块，不碰媒体。

use notecevia_core::board::{BlockKind, Body};

use crate::error::Error;
use crate::workspace::Workspace;

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

impl Workspace {
    /// 全部画板，按修改时间从新到旧；时间相同按编号大的在前。
    ///
    /// 单个画板读不懂时跳过它：一个坏文件不该让首页整个打不开。文件本身还在。
    pub fn list_boards(&self) -> Result<Vec<BoardInfo>, Error> {
        let mut boards = Vec::new();
        for id in self.board_ids()? {
            let Some(on_disk) = self.read_board_lenient(id)? else {
                continue;
            };
            let summary = on_disk.summary();
            boards.push(BoardInfo {
                id,
                title: summary.title,
                tags: summary.tags,
                updated_at: summary.updated_at,
                entry_count: summary.entry_count,
            });
        }
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

        let mut hits = Vec::new();
        for info in self.list_boards()? {
            let document = match self.load(info.id) {
                Ok(document) => document,
                Err(error @ Error::NewerFormat { .. }) => return Err(error),
                Err(_) => continue,
            };
            let title: Vec<char> = info.title.chars().collect();
            let mut snippet = find_chars(&title, &needle).map(|_| info.title.clone());
            if snippet.is_none() {
                snippet = info
                    .tags
                    .iter()
                    .find(|tag| find_chars(&chars_of(tag), &needle).is_some())
                    .map(|tag| format!("标签：{tag}"));
            }
            if snippet.is_none() {
                snippet = first_text_match(&document, &needle);
            }
            if let Some(snippet) = snippet {
                hits.push(SearchHit {
                    board_id: info.id,
                    title: info.title,
                    snippet,
                });
            }
        }
        Ok(hits)
    }
}

fn chars_of(text: &str) -> Vec<char> {
    text.chars().collect()
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
            .all(|(left, right)| left.to_lowercase().eq(right.to_lowercase()))
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
fn first_text_match(document: &notecevia_core::board::Document, needle: &[char]) -> Option<String> {
    for entry in &document.board.entries {
        if entry.kind != BlockKind::Text {
            continue;
        }
        let Some(Body::Text(text)) = document.body(entry.id) else {
            continue;
        };
        if let Some(snippet) = snippet_chars(&chars_of(&text.text), needle) {
            return Some(snippet);
        }
    }
    None
}
