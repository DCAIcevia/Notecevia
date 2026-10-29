//! 书架：一组册，存的是有序的册编号。
//!
//! 布局与册一致：`shelves/<id>.shelf`，CBOR，先写临时文件再改名，删除移进 `.trash`。
//! 架子只存引用：同一本册可以出现在多个书架上，删书架不删册。
//! 册被删掉后架子上会留下指向不存在册的编号：load 与 list 丢掉它们，不写回。

use std::{fs, path::PathBuf};

use notecevia_core::volume::{MIN_SHELF_VOLUMES, Shelf};

use crate::cbor::{self, Value};
use crate::error::Error;
use crate::format;
use crate::volumes::{FORMAT, checked_format, checked_kind, ids_in, ids_value, trashed_ids};
use crate::workspace::{Workspace, now_seconds, write_atomic};

/// 书架文件所在目录，放在工作区根下。
pub(crate) const SHELVES_DIR: &str = "shelves";
/// 文件后缀与回收站文件夹里的种类名。两处必须是同一个词。
const KIND: &str = "shelf";
/// 本格式认识的书架字段。其余字段读出来留着，写回时照抄。
const SHELF_KEYS: [&str; 5] = ["format", "kind", "title", "volumes", "updated_at"];

/// 列表页的一行。册序不在这里，打开才读。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShelfInfo {
    pub id: u64,
    pub title: String,
    /// 架子上还存在的册数。指向已删册的编号不算。
    pub volume_count: usize,
    /// Unix 秒。
    pub updated_at: i64,
}

/// 从书架文件读出来的东西：架子本身、修改时间，以及认不出的字段。
pub(crate) struct ShelfOnDisk {
    pub shelf: Shelf,
    pub updated_at: i64,
    /// 认不出的书架字段，写回时原样照抄。
    pub extras: Vec<(Value, Value)>,
}

fn shelf_value(shelf: &Shelf, updated_at: i64, extras: &[(Value, Value)]) -> Value {
    Value::map_with_extras(
        vec![
            ("format", Value::Integer(FORMAT)),
            ("kind", Value::text(KIND)),
            ("title", Value::text(shelf.title.clone())),
            ("volumes", ids_value(&shelf.volumes)),
            ("updated_at", Value::Integer(updated_at)),
        ],
        extras.to_vec(),
    )
}

fn shelf_from(value: &Value) -> Result<ShelfOnDisk, Error> {
    checked_format(value)?;
    checked_kind(value, KIND)?;
    let title = value
        .get("title")
        .and_then(Value::as_text)
        .ok_or(Error::MissingField("title"))?;
    let items = value
        .get("volumes")
        .and_then(Value::as_array)
        .ok_or(Error::MissingField("volumes"))?;
    let mut volumes: Vec<u64> = Vec::new();
    for item in items {
        // 坏掉的编号只丢这一个，其余照读。
        let Some(volume) = item
            .as_integer()
            .and_then(|number| u64::try_from(number).ok())
        else {
            continue;
        };
        // 文件被手改过可能出现重复，同一个编号只留第一次。
        if !volumes.contains(&volume) {
            volumes.push(volume);
        }
    }
    let updated_at = value
        .get("updated_at")
        .and_then(Value::as_integer)
        .unwrap_or(0);
    Ok(ShelfOnDisk {
        shelf: Shelf {
            title: title.to_owned(),
            volumes,
        },
        updated_at,
        extras: format::unknown_fields(value, &SHELF_KEYS),
    })
}

impl Workspace {
    /// 全部书架，按修改时间倒序，同秒按编号倒序。
    ///
    /// 读不懂的文件跳过：文件还在盘上，[Workspace::load_shelf] 会报出具体错误，
    /// [Workspace::next_shelf] 也不会复用它的编号。更高版本写的文件仍然报错。
    pub fn list_shelves(&self) -> Result<Vec<ShelfInfo>, Error> {
        let mut infos = Vec::new();
        for id in self.shelf_ids()? {
            let Some(on_disk) = self.read_shelf_lenient(id)? else {
                continue;
            };
            infos.push(ShelfInfo {
                id,
                title: on_disk.shelf.title,
                volume_count: on_disk
                    .shelf
                    .volumes
                    .iter()
                    .filter(|volume| self.volume_exists(**volume))
                    .count(),
                updated_at: on_disk.updated_at,
            });
        }
        infos.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(infos)
    }

    /// 读一个书架。文件不在时给 Io(NotFound)；指向已删册的编号在这里丢掉，不写回。
    pub fn load_shelf(&self, id: u64) -> Result<Shelf, Error> {
        let bytes = fs::read(self.shelf_path(id))?;
        let value = cbor::decode(&bytes)?;
        let mut on_disk = shelf_from(&value)?;
        on_disk
            .shelf
            .volumes
            .retain(|volume| self.volume_exists(*volume));
        Ok(on_disk.shelf)
    }

    /// 存盘。修改时间取当前时间；测试与导入要固定时间时用 [Workspace::save_shelf_at]。
    pub fn save_shelf(&self, id: u64, shelf: &Shelf) -> Result<(), Error> {
        self.save_shelf_at(id, shelf, now_seconds())
    }

    /// 存盘并写明修改时间。整份重写，先写临时文件再改名。
    pub fn save_shelf_at(&self, id: u64, shelf: &Shelf, updated_at: i64) -> Result<(), Error> {
        // 旧文件里认不出的字段留到写回；更高版本写的文件不覆盖，让用户先升级程序。
        let extras = match self.read_shelf(id) {
            Ok(Some(on_disk)) => on_disk.extras,
            Ok(None) => Vec::new(),
            Err(error @ Error::NewerFormat { .. }) => return Err(error),
            Err(_) => Vec::new(),
        };
        let value = shelf_value(shelf, updated_at, &extras);
        write_atomic(&self.shelf_path(id), &cbor::encode(&value))
    }

    /// 删一个书架：只把书架文件移进回收站，册文件一个都不动。
    pub fn delete_shelf(&self, id: u64) -> Result<(), Error> {
        let path = self.shelf_path(id);
        if !path.exists() {
            return Ok(());
        }
        let folder = format!("{}-{KIND}-{id}", now_seconds());
        self.move_to_trash(&path, &folder)
    }

    /// 下一个可用的书架编号。删过的号不再分配，和画板一样。
    pub fn next_shelf(&self) -> Result<u64, Error> {
        let max = self
            .shelf_ids()?
            .into_iter()
            .chain(trashed_ids(self.root(), KIND))
            .max();
        Ok(max.map_or(1, |id| id + 1))
    }

    /// 把一本册放到书架的末尾。一本册只在一层手动书架上：原来在别的架上先摘掉，
    /// 摘空的那层进回收站。册不存在时什么也不做。
    pub fn shelve_volume(&self, shelf_id: u64, volume: u64) -> Result<(), Error> {
        if !self.volume_exists(volume) {
            return Ok(());
        }
        let stamp = now_seconds();
        let mut shelf = self.load_shelf(shelf_id)?;
        if shelf.volumes.contains(&volume) {
            return Ok(());
        }
        self.detach_volume_from_shelves(volume, Some(shelf_id), stamp)?;
        shelf.insert_volume(usize::MAX, volume);
        self.save_shelf_at(shelf_id, &shelf, stamp)
    }

    /// 把一本册从书架上拿掉。拿空之后书架进回收站。
    pub fn unshelve_volume(&self, shelf_id: u64, volume: u64) -> Result<(), Error> {
        let stamp = now_seconds();
        let mut shelf = self.load_shelf(shelf_id)?;
        if !shelf.remove_volume(volume) {
            return Ok(());
        }
        if shelf.volumes.is_empty() {
            return self.delete_shelf(shelf_id);
        }
        self.save_shelf_at(shelf_id, &shelf, stamp)
    }

    /// 用两本以上的册合成一层书架。不足两本给 [Error::ShelfTooSmall]。
    /// 这些册原来在别的架上时先摘掉，摘空的架子进回收站。
    pub fn create_shelf_with(&self, title: String, volumes: Vec<u64>) -> Result<ShelfInfo, Error> {
        let stamp = now_seconds();
        let mut held: Vec<u64> = Vec::new();
        for volume in volumes {
            if self.volume_exists(volume) && !held.contains(&volume) {
                held.push(volume);
            }
        }
        if held.len() < MIN_SHELF_VOLUMES {
            return Err(Error::ShelfTooSmall);
        }
        let id = self.next_shelf()?;
        for volume in &held {
            self.detach_volume_from_shelves(*volume, None, stamp)?;
        }
        let shelf = Shelf {
            title,
            volumes: held,
        };
        let volume_count = shelf.volumes.len();
        self.save_shelf_at(id, &shelf, stamp)?;
        Ok(ShelfInfo {
            id,
            title: shelf.title,
            volume_count,
            updated_at: stamp,
        })
    }

    /// 形如 `shelves/<id>.shelf` 的路径。
    pub(crate) fn shelf_path(&self, id: u64) -> PathBuf {
        self.root().join(SHELVES_DIR).join(format!("{id}.{KIND}"))
    }

    pub(crate) fn shelf_ids(&self) -> Result<Vec<u64>, Error> {
        ids_in(&self.root().join(SHELVES_DIR), KIND)
    }

    /// 读书架文件。文件不在给 None；内容读不懂是错误。
    pub(crate) fn read_shelf(&self, id: u64) -> Result<Option<ShelfOnDisk>, Error> {
        let bytes = match fs::read(self.shelf_path(id)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let value = cbor::decode(&bytes)?;
        Ok(Some(shelf_from(&value)?))
    }

    /// 列表用的宽松版：单个书架读不懂只跳过它自己；更高版本的格式仍然要报错。
    pub(crate) fn read_shelf_lenient(&self, id: u64) -> Result<Option<ShelfOnDisk>, Error> {
        match self.read_shelf(id) {
            Ok(shelf) => Ok(shelf),
            Err(error @ Error::NewerFormat { .. }) => Err(error),
            Err(_) => Ok(None),
        }
    }

    /// 册文件还在不在。架子上指向已删册的编号按不存在处理。
    pub(crate) fn volume_exists(&self, volume: u64) -> bool {
        self.volume_path(volume).is_file()
    }
}
