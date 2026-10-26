//! 工作区：一个目录，里面放画板文件、块文件、媒体池与回收站。
//!
//! 布局与字段名按 docs/design/format.md：画板一张一个文件，正文按块编号单独存，
//! 目录项用 block_ref 指向块（工作区内是相对路径），媒体按内容哈希进媒体池。
//!
//! 写盘顺序是先块后画板。中途失败最多留下没人引用的块文件，不会留下指向空处的
//! 目录项；这些孤儿文件在保存成功后移进回收站，不直接删。媒体不回收——
//! 同一份图可能还被别的画板引用。

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use notecevia_core::board::{Body, Document};

use crate::cbor;
use crate::digest;
use crate::format::{self, BoardOnDisk};
use crate::paths;

pub use crate::error::Error;

const BOARDS_DIR: &str = "boards";
const BLOCKS_DIR: &str = "blocks";
const MEDIA_DIR: &str = "media";
/// 回收站目录名。放在工作区里，整个工作区拷走时一起带走。
const TRASH_DIR: &str = ".trash";

/// 一个工作区目录。
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    /// 打开或建立工作区。目录不存在就建，已有的内容不动。
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, Error> {
        let workspace = Self { root: root.into() };
        for dir in [BOARDS_DIR, BLOCKS_DIR, MEDIA_DIR] {
            fs::create_dir_all(workspace.root.join(dir))?;
        }
        Ok(workspace)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 最近改过的画板。没有画板时返回 None，第一次启动就是这种情况。
    pub fn load_latest(&self) -> Result<Option<(u64, Document)>, Error> {
        let mut latest: Option<(i64, u64)> = None;
        for id in self.board_ids()? {
            let Some(on_disk) = self.read_board_lenient(id)? else {
                continue;
            };
            let better = match latest {
                Some((best_at, best_id)) => (on_disk.updated_at, id) > (best_at, best_id),
                None => true,
            };
            if better {
                latest = Some((on_disk.updated_at, id));
            }
        }
        let Some((_, id)) = latest else {
            return Ok(None);
        };
        Ok(Some((id, self.load(id)?)))
    }

    /// 下一个可用的画板编号。删过的不复用，免得旧文件被新内容顶替。
    pub fn next_board(&self) -> Result<u64, Error> {
        Ok(self
            .board_ids()?
            .into_iter()
            .max()
            .map_or(1, |id: u64| id + 1))
    }

    /// 读一张画板与它引用的正文。画板文件不存在时给一张空画板。
    ///
    /// 单个块文件坏了只丢那一段正文，位置留着，用户能重新写。画板文件本身坏了才是
    /// 真错误——那种情况宁可报错让人看见，也不要拿空画板覆盖掉。
    pub fn load(&self, board_id: u64) -> Result<Document, Error> {
        let Some(on_disk) = self.read_board(board_id)? else {
            return Ok(Document::new("画板"));
        };
        let bodies = self.load_bodies(board_id, &on_disk);
        Ok(Document {
            board: on_disk.board,
            bodies,
        })
    }

    /// 存盘。修改时间取当前时间；测试与导入要固定时间时用 [save_at]。
    pub fn save(&self, board_id: u64, document: &Document) -> Result<(), Error> {
        self.save_at(board_id, document, now_seconds())
    }

    /// 存盘并写明修改时间。先写块再写画板，每个文件都是先写临时文件再改名。
    pub fn save_at(
        &self,
        board_id: u64,
        document: &Document,
        updated_at: i64,
    ) -> Result<(), Error> {
        for entry in &document.board.entries {
            let Some(body) = document.body(entry.id) else {
                continue;
            };
            // 种类对不上的正文不写：宁可丢这一段，也不写出自相矛盾的文件。
            if !format::body_matches(entry.kind, body) {
                continue;
            }
            let value = format::body_value(entry.kind, entry.id, body);
            write_atomic(&self.block_path(board_id, entry.id), &cbor::encode(&value))?;
        }
        let reference = |id: u64| self.block_ref(board_id, id);
        let value = format::board_value(&document.board, updated_at, reference);
        write_atomic(&self.board_path(board_id), &cbor::encode(&value))?;
        self.reclaim_blocks(board_id, document, updated_at)?;
        Ok(())
    }

    /// 删掉一张画板：画板文件与它的块文件一起移进回收站，媒体留着。
    pub fn delete_board(&self, board_id: u64) -> Result<(), Error> {
        let board = self.board_path(board_id);
        let blocks: Vec<PathBuf> = self
            .block_ids_of(board_id)?
            .into_iter()
            .map(|block_id| self.block_path(board_id, block_id))
            .collect();
        if !board.exists() && blocks.is_empty() {
            return Ok(());
        }
        let folder = format!("{}-board-{board_id}", now_seconds());
        if board.exists() {
            self.move_to_trash(&board, &folder)?;
        }
        for path in blocks {
            self.move_to_trash(&path, &folder)?;
        }
        Ok(())
    }

    /// 把一段内容放进媒体池，返回内容哈希。同样的内容只占一份文件。
    ///
    /// 原文件名由块自己记着，这里用不到；盘上只认哈希。
    pub fn put_media(&self, _name: &str, bytes: &[u8]) -> Result<String, Error> {
        let hash = digest::hex(&digest::sha256(bytes));
        let path = self.root.join(MEDIA_DIR).join(&hash);
        if !path.exists() {
            write_atomic(&path, bytes)?;
        }
        Ok(hash)
    }

    /// 按哈希读媒体内容。哈希格式不对或文件不在都给错误，调用方不用猜。
    pub fn media_bytes(&self, media: &str) -> Result<Vec<u8>, Error> {
        let path = self
            .media_file(media)
            .ok_or_else(|| Error::BadMediaName(media.to_owned()))?;
        Ok(fs::read(path)?)
    }

    /// 媒体文件本机路径。交给系统默认程序打开时用。
    ///
    /// 哈希不是六十四位十六进制时给媒体目录本身，而不是把原始输入拼进路径：
    /// 这样即使块文件被改过，也只会打开失败，不会打开工作区外的文件。
    pub fn media_path(&self, media: &str) -> PathBuf {
        self.media_file(media)
            .unwrap_or_else(|| self.root.join(MEDIA_DIR))
    }

    /// 哈希合法时给媒体文件的真实路径；不合法给 None。内部用它挡路径逃逸。
    pub(crate) fn media_file(&self, media: &str) -> Option<PathBuf> {
        hash_file_name(media).map(|hash| self.root.join(MEDIA_DIR).join(hash))
    }

    /// 清掉工作区里上次崩溃留下的 *.tmp，返回清掉的条数。启动时调一次。
    pub fn recover(&self) -> Result<usize, Error> {
        let mut removed = 0;
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            // 目录可能在扫描途中被别的进程删掉；跳过，不为此中断启动。
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                let path = entry.path();
                if file_type.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|ext| ext.to_str()) == Some("tmp")
                    && fs::remove_file(&path).is_ok()
                {
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    pub(crate) fn board_path(&self, board_id: u64) -> PathBuf {
        self.root.join(BOARDS_DIR).join(format!("{board_id}.board"))
    }

    /// 画板文件在容器与 block_ref 里的写法：工作区内，正斜杠，跨平台一致。
    pub(crate) fn board_ref(&self, board_id: u64) -> String {
        format!("{BOARDS_DIR}/{board_id}.board")
    }

    /// 目录项里记的块地址。块编号只在画板内唯一，所以文件名带上画板号。
    pub(crate) fn block_ref(&self, board_id: u64, block_id: u64) -> String {
        format!("{BLOCKS_DIR}/{board_id}-{block_id}.block")
    }

    pub(crate) fn block_path(&self, board_id: u64, block_id: u64) -> PathBuf {
        self.root
            .join(BLOCKS_DIR)
            .join(format!("{board_id}-{block_id}.block"))
    }

    /// 把画板里记的地址变成本机路径。绝对路径和 .. 一律拒绝。
    pub(crate) fn resolve(&self, reference: &str) -> Option<PathBuf> {
        paths::inside(&self.root, reference)
    }

    /// 读画板文件。文件不在给 None；内容读不懂是错误。
    pub(crate) fn read_board(&self, board_id: u64) -> Result<Option<BoardOnDisk>, Error> {
        let bytes = match fs::read(self.board_path(board_id)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let value = cbor::decode(&bytes)?;
        Ok(Some(format::board_from(&value)?))
    }

    /// 列表与搜索用的宽松版：单个画板读不懂只跳过它自己；
    /// 更高版本的格式仍然要报错，否则用户会以为文件丢了。
    pub(crate) fn read_board_lenient(&self, board_id: u64) -> Result<Option<BoardOnDisk>, Error> {
        match self.read_board(board_id) {
            Ok(board) => Ok(board),
            Err(error @ Error::NewerFormat { .. }) => Err(error),
            Err(_) => Ok(None),
        }
    }

    /// 按目录项的 block_ref 读正文。没有 block_ref 的旧文件回退到按编号拼地址。
    pub(crate) fn load_bodies(&self, board_id: u64, on_disk: &BoardOnDisk) -> BTreeMap<u64, Body> {
        let mut bodies = BTreeMap::new();
        for entry in &on_disk.board.entries {
            let reference = on_disk
                .refs
                .get(&entry.id)
                .cloned()
                .unwrap_or_else(|| self.block_ref(board_id, entry.id));
            let Some(path) = self.resolve(&reference) else {
                continue;
            };
            let Ok(bytes) = fs::read(path) else {
                continue;
            };
            let Ok(value) = cbor::decode(&bytes) else {
                continue;
            };
            let Ok(body) = format::body_from(&value, entry.kind) else {
                continue;
            };
            bodies.insert(entry.id, body);
        }
        bodies
    }

    /// 保存成功后把本画板不再被目录项引用的块文件移进回收站。
    fn reclaim_blocks(&self, board_id: u64, document: &Document, stamp: i64) -> Result<(), Error> {
        let kept: BTreeSet<u64> = document
            .board
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect();
        let folder = format!("{stamp}-blocks");
        for block_id in self.block_ids_of(board_id)? {
            if kept.contains(&block_id) {
                continue;
            }
            self.move_to_trash(&self.block_path(board_id, block_id), &folder)?;
        }
        Ok(())
    }

    /// 把文件移进 .trash/<folder>/。同名的已经在了就加序号，不覆盖回收站里的东西。
    fn move_to_trash(&self, path: &Path, folder: &str) -> Result<(), Error> {
        if !path.exists() {
            return Ok(());
        }
        let trash = self.root.join(TRASH_DIR).join(folder);
        fs::create_dir_all(&trash)?;
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return Ok(());
        };
        let mut target = trash.join(name);
        let mut attempt = 1;
        while target.exists() {
            target = trash.join(format!("{name}.{attempt}"));
            attempt += 1;
        }
        fs::rename(path, target)?;
        Ok(())
    }

    pub(crate) fn board_ids(&self) -> Result<Vec<u64>, Error> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(self.root.join(BOARDS_DIR))? {
            let path = entry?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("board") {
                continue;
            }
            if let Some(id) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.parse::<u64>().ok())
            {
                ids.push(id);
            }
        }
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    /// 本画板的块文件编号。文件名不规范的直接忽略。
    pub(crate) fn block_ids_of(&self, board_id: u64) -> Result<Vec<u64>, Error> {
        let prefix = format!("{board_id}-");
        let mut ids = Vec::new();
        for entry in fs::read_dir(self.root.join(BLOCKS_DIR))? {
            let path = entry?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("block") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let Some(rest) = stem.strip_prefix(&prefix) else {
                continue;
            };
            if let Ok(id) = rest.parse::<u64>() {
                ids.push(id);
            }
        }
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }
}

/// 先写临时文件、落盘后再改名。改名是同目录的原子操作，
/// 所以断电最多丢掉这一次的修改，不会留下半个文件。
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let temp = path.with_extension("tmp");
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temp, path)?;
    Ok(())
}

/// 媒体文件名就是内容哈希：六十四位十六进制，读的时候大小写不敏感。
fn hash_file_name(media: &str) -> Option<String> {
    let hash = media.to_ascii_lowercase();
    if hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(hash)
    } else {
        None
    }
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
