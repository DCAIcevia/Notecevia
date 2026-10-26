//! 容器的打包与拆包：一张画板加它用到的块文件与媒体，打进一个文件带走。
//!
//! 导入时给画板换一个新编号，块文件按新编号重命名，画板里的 block_ref 跟着改。
//! 媒体按哈希合并：容器里的媒体已经在本机媒体池里就不重复写。

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::cbor;
use crate::container::{self, ContainerEntry};
use crate::error::Error;
use crate::format;
use crate::workspace::{Workspace, write_atomic};

impl Workspace {
    /// 把一张画板打包成一个容器文件。
    pub fn export_container(&self, board_id: u64, path: impl AsRef<Path>) -> Result<(), Error> {
        let Some(on_disk) = self.read_board(board_id)? else {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("画板 {board_id} 不存在"),
            )));
        };
        let bodies = self.load_bodies(board_id, &on_disk);

        let mut entries = vec![ContainerEntry {
            path: self.board_ref(board_id),
            bytes: fs::read(self.board_path(board_id))?,
        }];
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
            entries.push(ContainerEntry {
                path: reference,
                bytes,
            });
        }

        let mut media = BTreeSet::new();
        for body in bodies.values() {
            if let Some(hash) = format::body_media(body) {
                media.insert(hash.to_owned());
            }
        }
        for hash in media {
            let Some(path) = self.media_file(&hash) else {
                continue;
            };
            let Ok(bytes) = fs::read(path) else {
                continue;
            };
            entries.push(ContainerEntry {
                path: format!("media/{hash}"),
                bytes,
            });
        }

        write_atomic(path.as_ref(), &container::encode(&entries))
    }

    /// 把一个容器文件导进来，返回新画板的编号。
    pub fn import_container(&self, path: impl AsRef<Path>) -> Result<u64, Error> {
        let entries = container::decode(&fs::read(path)?)?;
        let mut board_bytes: Option<Vec<u8>> = None;
        let mut old_board_id = 0;
        for entry in &entries {
            let Some(name) = entry.path.strip_prefix("boards/") else {
                continue;
            };
            let Some(stem) = name.strip_suffix(".board") else {
                continue;
            };
            if board_bytes.is_some() {
                return Err(Error::BadContainer("一个容器只能装一张画板"));
            }
            old_board_id = stem.parse::<u64>().unwrap_or(0);
            board_bytes = Some(entry.bytes.clone());
        }
        let Some(board_bytes) = board_bytes else {
            return Err(Error::BadContainer("容器里没有画板文件"));
        };
        let on_disk = format::board_from(&cbor::decode(&board_bytes)?)?;
        let new_board_id = self.next_board()?;

        // 先写块，再写媒体，最后写画板：中途失败最多留下没人引用的文件。
        for entry in &on_disk.board.entries {
            let reference = on_disk
                .refs
                .get(&entry.id)
                .cloned()
                .unwrap_or_else(|| format!("blocks/{old_board_id}-{}.block", entry.id));
            let Some(source) = entries.iter().find(|item| item.path == reference) else {
                continue;
            };
            write_atomic(&self.block_path(new_board_id, entry.id), &source.bytes)?;
        }
        for entry in &entries {
            let Some(hash) = entry.path.strip_prefix("media/") else {
                continue;
            };
            let Some(target) = self.media_file(hash) else {
                continue;
            };
            if !target.exists() {
                write_atomic(&target, &entry.bytes)?;
            }
        }
        let reference = |id: u64| self.block_ref(new_board_id, id);
        let value = format::board_value(&on_disk.board, on_disk.updated_at, reference);
        write_atomic(&self.board_path(new_board_id), &cbor::encode(&value))?;
        Ok(new_board_id)
    }
}
