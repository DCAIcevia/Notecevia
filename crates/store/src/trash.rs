#![forbid(unsafe_code)]

//! 回收站：列出、还原、彻底删除。
//!
//! 东西是怎么进 `.trash/` 的由 `workspace` 决定，这里只管把里面的读出来、搬回原处、
//! 或者真删掉。条目是 `.trash/` 下一层的目录，名字是 `<删除时间戳>-<种类>-<编号>`。
//!
//! 页文件里不记归属，所以还原一页只会把它放回列表，不会自动回到原来那本册。

use std::fs;
use std::path::{Path, PathBuf};

use crate::cbor::{self, Value};
use crate::error::Error;
use crate::format;
use crate::workspace::{
    BLOCKS_DIR, BOARDS_DIR, THUMBNAILS_DIR, TRASH_DIR, Workspace, remove_trash_entry,
    trash_entry_deleted_at,
};

/// 回收站留多久：30 天。界面要说清"还剩几天"，所以这个数对外。
pub const TRASH_KEEP_DAYS: i64 = 30;

/// 一天的秒数。保留期与"还剩几天"都按它算，别在两处各写一遍。
pub(crate) const SECONDS_PER_DAY: i64 = 24 * 60 * 60;

/// 回收站里一条东西的种类。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrashKind {
    Board,
    Volume,
    Shelf,
}

/// 回收站里的一条。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashItem {
    /// `.trash/` 下的目录名。还原与彻底删除都按它定位。
    pub folder: String,
    pub kind: TrashKind,
    /// 原来的编号。还原时就放回这个编号。
    pub id: u64,
    /// 名字。文件读不出来时是空串，界面自己给占位。
    pub title: String,
    /// 删掉的时刻，Unix 秒。
    pub deleted_at: i64,
    /// 这条占多少字节。
    pub bytes: u64,
}

/// 一条还剩几天到期。已经到期给 0：到 0 就该清掉了。
pub fn days_left(deleted_at: i64, now: i64) -> i64 {
    let elapsed = (now - deleted_at).max(0);
    (TRASH_KEEP_DAYS - elapsed / SECONDS_PER_DAY).max(0)
}

impl Workspace {
    /// 回收站里能还原的东西，按删除时间从新到旧。
    ///
    /// 只列笔记、册、书架：孤儿块（`<时间戳>-blocks`）是保存时回收的中间产物，不往
    /// 用户面前摆。名字读不出来的条目也跳过——还原不了的东西不该出现在能还原的列表里。
    pub fn list_trash(&self) -> Result<Vec<TrashItem>, Error> {
        let trash = self.root().join(TRASH_DIR);
        let entries = match fs::read_dir(&trash) {
            Ok(entries) => entries,
            // 从没删过东西的工作区没有 .trash，不算错误。
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut items = Vec::new();
        for entry in entries.flatten() {
            let Some(folder) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some((kind, id)) = parse_folder(&folder) else {
                continue;
            };
            let Some(deleted_at) = trash_entry_deleted_at(&entry) else {
                continue;
            };
            let path = entry.path();
            let Some(title) = read_title(&path, kind, id) else {
                continue;
            };
            items.push(TrashItem {
                folder,
                kind,
                id,
                title,
                deleted_at,
                bytes: folder_bytes(&path),
            });
        }
        items.sort_by(|left, right| {
            right
                .deleted_at
                .cmp(&left.deleted_at)
                .then_with(|| right.folder.cmp(&left.folder))
        });
        Ok(items)
    }

    /// 把一条搬回原处；笔记顺手补进索引，列表立刻能看到。
    ///
    /// 编号被占用时拒绝：宁可让人看到一句说明，也不要把两条笔记写成同一个文件。
    /// 先看清楚每个目标都在不在，再动手搬——不存在搬到一半才发现撞车。
    pub fn restore_trash(&self, folder: &str) -> Result<(), Error> {
        let (kind, id) =
            parse_folder(folder).ok_or_else(|| Error::BadTrashFolder(folder.to_owned()))?;
        let dir = self.trash_folder(folder)?;
        if !dir.is_dir() {
            return Err(Error::BadTrashFolder(folder.to_owned()));
        }
        let moves = match kind {
            TrashKind::Board => board_moves(&dir, self.root(), id)?,
            TrashKind::Volume => vec![(
                dir.join(format!("{id}.volume")),
                self.root()
                    .join(crate::volumes::VOLUMES_DIR)
                    .join(format!("{id}.volume")),
            )],
            TrashKind::Shelf => vec![(
                dir.join(format!("{id}.shelf")),
                self.root()
                    .join(crate::shelves::SHELVES_DIR)
                    .join(format!("{id}.shelf")),
            )],
        };
        for (_, target) in &moves {
            if target.exists() {
                return Err(Error::TrashTargetExists(id));
            }
        }
        for (source, target) in &moves {
            if source.exists() {
                fs::rename(source, target)?;
            }
        }
        if kind == TrashKind::Board {
            self.remember_board_from_disk(id)?;
        }
        remove_trash_entry(&dir);
        Ok(())
    }

    /// 彻底删掉一条，不等保留期。
    pub fn purge_trash_item(&self, folder: &str) -> Result<(), Error> {
        let dir = self.trash_folder(folder)?;
        if !dir.exists() {
            return Ok(());
        }
        if remove_trash_entry(&dir) {
            Ok(())
        } else {
            Err(Error::Io(std::io::Error::other(format!(
                "删不掉回收站里的 {folder}"
            ))))
        }
    }

    /// 清空回收站，返回删掉的条数。删不掉的跳过：一个删不掉的文件不该挡住其它。
    pub fn empty_trash(&self) -> Result<usize, Error> {
        let trash = self.root().join(TRASH_DIR);
        let entries = match fs::read_dir(&trash) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(error.into()),
        };
        let mut removed = 0;
        for entry in entries.flatten() {
            if remove_trash_entry(&entry.path()) {
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// `.trash/<folder>` 的路径。带分隔符、或是 `.`、`..` 的名字一律拒绝。
    fn trash_folder(&self, folder: &str) -> Result<PathBuf, Error> {
        if folder.is_empty() || folder == "." || folder == ".." || folder.contains(['/', '\\']) {
            return Err(Error::BadTrashFolder(folder.to_owned()));
        }
        Ok(self.root().join(TRASH_DIR).join(folder))
    }
}

/// 目录名里的种类与编号：`<时间戳>-<种类>-<编号>`。别的写法不认。
fn parse_folder(folder: &str) -> Option<(TrashKind, u64)> {
    let mut parts = folder.rsplit('-');
    let id = parts.next()?.parse::<u64>().ok()?;
    let kind = match parts.next()? {
        "board" => TrashKind::Board,
        "volume" => TrashKind::Volume,
        "shelf" => TrashKind::Shelf,
        _ => return None,
    };
    Some((kind, id))
}

/// 一条笔记要搬回去的东西：画板、它的块、它的缩略图。扩展名决定去处。
fn board_moves(dir: &Path, root: &Path, id: u64) -> Result<Vec<(PathBuf, PathBuf)>, Error> {
    let main = format!("{id}.board");
    let mut moves = Vec::new();
    let mut has_board = false;
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name() else {
            continue;
        };
        if name.to_str() == Some(main.as_str()) {
            has_board = true;
        }
        let target = match path.extension().and_then(|ext| ext.to_str()) {
            Some("board") => root.join(BOARDS_DIR).join(name),
            Some("block") => root.join(BLOCKS_DIR).join(name),
            Some("png") => root.join(THUMBNAILS_DIR).join(name),
            // 不认识的留在回收站里，不删也不搬。
            _ => continue,
        };
        moves.push((path, target));
    }
    // 画板文件是这条的主件；没有它就不算一条能还原的笔记。
    if !has_board {
        return Err(Error::BadTrashFolder(dir.display().to_string()));
    }
    Ok(moves)
}

/// 读一条的名字。读不出来就说明这条还原不了。
fn read_title(dir: &Path, kind: TrashKind, id: u64) -> Option<String> {
    let path = match kind {
        TrashKind::Board => dir.join(format!("{id}.board")),
        TrashKind::Volume => dir.join(format!("{id}.volume")),
        TrashKind::Shelf => dir.join(format!("{id}.shelf")),
    };
    let bytes = fs::read(path).ok()?;
    let value = cbor::decode(&bytes).ok()?;
    match kind {
        TrashKind::Board => format::board_from(&value)
            .ok()
            .map(|on_disk| on_disk.board.title),
        _ => value
            .get("title")
            .and_then(Value::as_text)
            .map(str::to_owned),
    }
}

/// 目录里这一层文件一共多少字节。
fn folder_bytes(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .map(|metadata| metadata.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = SECONDS_PER_DAY;

    #[test]
    fn days_left_counts_down_and_stops_at_zero() {
        assert_eq!(days_left(0, 0), TRASH_KEEP_DAYS);
        assert_eq!(days_left(0, DAY), TRASH_KEEP_DAYS - 1);
        assert_eq!(days_left(0, 30 * DAY), 0);
        assert_eq!(days_left(0, 99 * DAY), 0, "过期的停在 0，不给负数");
        assert_eq!(days_left(DAY, 0), TRASH_KEEP_DAYS, "时钟回拨不出现负数天数");
    }

    #[test]
    fn folder_names_are_read_from_the_right() {
        assert_eq!(
            parse_folder("1700000000-board-7"),
            Some((TrashKind::Board, 7))
        );
        assert_eq!(
            parse_folder("1700000000-volume-2"),
            Some((TrashKind::Volume, 2))
        );
        assert_eq!(
            parse_folder("1700000000-shelf-3"),
            Some((TrashKind::Shelf, 3))
        );
        assert_eq!(
            parse_folder("1700000000-blocks"),
            None,
            "孤儿块不往用户面前摆"
        );
        assert_eq!(parse_folder("handmade"), None);
        assert_eq!(parse_folder("1700000000-board-abc"), None);
    }
}
