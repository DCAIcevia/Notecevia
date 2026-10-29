//! 册：一本书，存的是有序的页编号。
//!
//! 文件布局与画板一致：`volumes/<id>.volume`，CBOR 一格一个文件，先写临时文件再改名，
//! 删除移进 `.trash`。册的数量少，列表直接扫目录，不进 `index.cbor`。
//!
//! 页永远属于一个册：编号 0 的 Empty book 是默认册，工作区打开时建出来（标题用
//! [EMPTY_BOOK_TITLE]），不可删除。页文件里没有反向字段，一页属于哪本册只记在册文件的
//! `pages` 里，所以 **一个页最多属于一个册**，这条约束由这里的写入接口保证
//! （[Workspace::assign_page]）。除 Empty book 外，删册连页一起删：每页先走
//! [Workspace::delete_board] 进回收站，再删册文件，最后从所有书架里摘下这个编号；
//! 删页时也由 [Workspace::delete_board] 把编号从册里摘掉。
//!
//! 盘上存在、却不在任何册文件里的页算 Empty book 的页，见 [Workspace::pages_in_volume]。
//! 册文件损坏时它的编号读不出来，那些页就落进 Empty book——不会凭空消失，这是有意的。
//! 外部手改文件可能留下指向不存在页的编号：load 与 list 会丢掉它们，不写回。
//!
//! 册与书架共用的小工具（编号扫描、格式号、回收站编号）也放在这里，`shelves.rs` 引用。

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use notecevia_core::volume::{EMPTY_BOOK_ID, EMPTY_BOOK_TITLE, Volume};

use crate::cbor::{self, Value};
use crate::error::Error;
use crate::format;
use crate::workspace::{TRASH_DIR, Workspace, now_seconds, write_atomic};

/// 册文件所在目录，放在工作区根下。
pub(crate) const VOLUMES_DIR: &str = "volumes";
/// 文件后缀与回收站文件夹里的种类名。两处必须是同一个词。
const KIND: &str = "volume";
/// 册与书架文件的格式号。两种文件都从 1 起，`kind` 字段区分是哪一种。
pub(crate) const FORMAT: i64 = 1;
/// 本格式认识的册字段。其余字段读出来留着，写回时照抄。
const VOLUME_KEYS: [&str; 5] = ["format", "kind", "title", "pages", "updated_at"];

/// 列表页的一行。页序不在这里，打开才读。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeInfo {
    pub id: u64,
    pub title: String,
    /// 还在盘上的页数。指向已删页的编号不算。
    pub page_count: usize,
    /// Unix 秒。
    pub updated_at: i64,
}

/// 从册文件读出来的东西：册本身、修改时间，以及认不出的字段。
pub(crate) struct VolumeOnDisk {
    pub volume: Volume,
    pub updated_at: i64,
    /// 认不出的册字段，写回时原样照抄。
    pub extras: Vec<(Value, Value)>,
}

/// 编号表写成 CBOR 数组。
pub(crate) fn ids_value(ids: &[u64]) -> Value {
    Value::Array(
        ids.iter()
            .map(|&id| Value::Integer(format::integer_of(id)))
            .collect(),
    )
}

/// 读格式号。比本程序支持的更大就报 NewerFormat，让用户先升级程序。
pub(crate) fn checked_format(value: &Value) -> Result<i64, Error> {
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

/// 文件里记的种类要和打开它的方式一致，免得把册当书架读。
pub(crate) fn checked_kind(value: &Value, expected: &str) -> Result<(), Error> {
    match value.get("kind").and_then(Value::as_text) {
        Some(kind) if kind == expected => Ok(()),
        _ => Err(Error::WrongType("kind")),
    }
}

/// 目录里某一类文件的编号，升序去重。名字不是编号的文件直接忽略。
pub(crate) fn ids_in(dir: &Path, extension: &str) -> Result<Vec<u64>, Error> {
    let mut ids = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some(extension) {
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

/// 回收站里某一类的编号。目录名是 `<时间戳>-<种类>-<编号>`，别的种类不认。
///
/// 删过的编号不再分配靠这一步：册文件进了回收站，最大号还在，就不会被新册顶上。
pub(crate) fn trashed_ids(root: &Path, kind: &str) -> Vec<u64> {
    let mut ids = Vec::new();
    let Ok(entries) = fs::read_dir(root.join(TRASH_DIR)) else {
        return ids;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let mut parts = name.rsplit('-');
        let Some(id) = parts.next().and_then(|part| part.parse::<u64>().ok()) else {
            continue;
        };
        if parts.next() != Some(kind) {
            continue;
        }
        ids.push(id);
    }
    ids
}

fn volume_value(volume: &Volume, updated_at: i64, extras: &[(Value, Value)]) -> Value {
    Value::map_with_extras(
        vec![
            ("format", Value::Integer(FORMAT)),
            ("kind", Value::text(KIND)),
            ("title", Value::text(volume.title.clone())),
            ("pages", ids_value(&volume.pages)),
            ("updated_at", Value::Integer(updated_at)),
        ],
        extras.to_vec(),
    )
}

fn volume_from(value: &Value) -> Result<VolumeOnDisk, Error> {
    checked_format(value)?;
    checked_kind(value, KIND)?;
    let title = value
        .get("title")
        .and_then(Value::as_text)
        .ok_or(Error::MissingField("title"))?;
    let items = value
        .get("pages")
        .and_then(Value::as_array)
        .ok_or(Error::MissingField("pages"))?;
    let mut pages: Vec<u64> = Vec::new();
    for item in items {
        // 坏掉的编号只丢这一个，其余照读。
        let Some(page) = item
            .as_integer()
            .and_then(|number| u64::try_from(number).ok())
        else {
            continue;
        };
        // 文件被手改过可能出现重复，同一个编号只留第一次。
        if !pages.contains(&page) {
            pages.push(page);
        }
    }
    let updated_at = value
        .get("updated_at")
        .and_then(Value::as_integer)
        .unwrap_or(0);
    Ok(VolumeOnDisk {
        volume: Volume {
            title: title.to_owned(),
            pages,
        },
        updated_at,
        extras: format::unknown_fields(value, &VOLUME_KEYS),
    })
}

impl Workspace {
    /// 全部册：其余册按修改时间倒序、同秒按编号倒序，Empty book 固定排在最后一个。
    ///
    /// 读不懂的文件跳过：文件还在盘上，[Workspace::load_volume] 会报出具体错误，
    /// [Workspace::next_volume] 也不会复用它的编号。更高版本写的文件仍然报错，
    /// 否则用户会以为册丢了。Empty book 不会缺席：文件被外部删掉或读不懂时按空册列出，
    /// 名下的页按孤儿页计（[Workspace::pages_in_volume]）。
    pub fn list_volumes(&self) -> Result<Vec<VolumeInfo>, Error> {
        let mut infos = Vec::new();
        for id in self.volume_ids()? {
            if id == EMPTY_BOOK_ID {
                continue;
            }
            let Some(on_disk) = self.read_volume_lenient(id)? else {
                continue;
            };
            infos.push(VolumeInfo {
                id,
                title: on_disk.volume.title,
                page_count: on_disk
                    .volume
                    .pages
                    .iter()
                    .filter(|page| self.page_exists(**page))
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
        infos.push(self.empty_book_info()?);
        Ok(infos)
    }

    /// Empty book 在列表里的那一行。文件不在或读不懂时用常量标题兜底：
    /// 它是默认册，列表里不能缺，页数按孤儿页算。
    fn empty_book_info(&self) -> Result<VolumeInfo, Error> {
        let on_disk = self.read_volume_lenient(EMPTY_BOOK_ID)?;
        Ok(VolumeInfo {
            id: EMPTY_BOOK_ID,
            title: on_disk.as_ref().map_or_else(
                || EMPTY_BOOK_TITLE.to_owned(),
                |found| found.volume.title.clone(),
            ),
            page_count: self.pages_in_volume(EMPTY_BOOK_ID)?.len(),
            updated_at: on_disk.map_or(0, |found| found.updated_at),
        })
    }

    /// 建出 Empty book 的文件。已经在了就一个字节都不动。
    ///
    /// 文件损坏时也留着不动：修不了也别拿空册覆盖，那些页会按孤儿页出现在它下面。
    pub(crate) fn ensure_empty_book(&self) -> Result<(), Error> {
        if self.volume_path(EMPTY_BOOK_ID).exists() {
            return Ok(());
        }
        self.save_volume_at(EMPTY_BOOK_ID, &Volume::empty_book(), now_seconds())
    }

    /// 读一本册。文件不在时给 Io(NotFound)；指向已删页的编号在这里丢掉，不写回。
    pub fn load_volume(&self, id: u64) -> Result<Volume, Error> {
        let bytes = fs::read(self.volume_path(id))?;
        let value = cbor::decode(&bytes)?;
        let mut on_disk = volume_from(&value)?;
        on_disk.volume.pages.retain(|page| self.page_exists(*page));
        Ok(on_disk.volume)
    }

    /// 存盘。修改时间取当前时间；测试与导入要固定时间时用 [Workspace::save_volume_at]。
    pub fn save_volume(&self, id: u64, volume: &Volume) -> Result<(), Error> {
        self.save_volume_at(id, volume, now_seconds())
    }

    /// 存盘并写明修改时间。整份重写，先写临时文件再改名。
    pub fn save_volume_at(&self, id: u64, volume: &Volume, updated_at: i64) -> Result<(), Error> {
        // 旧文件里认不出的字段留到写回；更高版本写的文件不覆盖，让用户先升级程序。
        let extras = match self.read_volume(id) {
            Ok(Some(on_disk)) => on_disk.extras,
            Ok(None) => Vec::new(),
            Err(error @ Error::NewerFormat { .. }) => return Err(error),
            Err(_) => Vec::new(),
        };
        let value = volume_value(volume, updated_at, &extras);
        write_atomic(&self.volume_path(id), &cbor::encode(&value))
    }

    /// 删一本册：册拥有页，所以连页一起删。
    ///
    /// Empty book 不能删，给它 [Error::CannotDeleteEmptyBook]。其余册按这个顺序删：
    /// 逐页走 [Workspace::delete_board]（进回收站、从所有册里摘掉），删册文件，
    /// 最后把册编号从所有书架里摘掉。书架本身不删。删页是进回收站，用户还能找回。
    pub fn delete_volume(&self, id: u64) -> Result<(), Error> {
        if id == EMPTY_BOOK_ID {
            return Err(Error::CannotDeleteEmptyBook);
        }
        let stamp = now_seconds();
        match self.read_volume(id) {
            Ok(Some(on_disk)) => {
                for page in on_disk.volume.pages {
                    self.delete_board(page)?;
                }
            }
            Ok(None) => {}
            // 更高版本写的文件看不懂，先升级程序，别动它的页。
            Err(error @ Error::NewerFormat { .. }) => return Err(error),
            // 文件坏了读不出页：页按孤儿页留在 Empty book 里，不凭空消失。
            Err(_) => {}
        }
        let path = self.volume_path(id);
        if path.exists() {
            let folder = format!("{stamp}-{KIND}-{id}");
            self.move_to_trash(&path, &folder)?;
        }
        self.detach_volume_from_shelves(id, None, stamp)
    }

    /// 把一个册编号从书架上摘掉，`keep` 那层除外。摘空的架子跟着进回收站：
    /// 一层书架要有两本册才成立，零本就不该留着。读不懂的书架跳过它自己：
    /// 架子上指向已删册的编号本来也会在读的时候丢掉。
    pub(crate) fn detach_volume_from_shelves(
        &self,
        volume: u64,
        keep: Option<u64>,
        stamp: i64,
    ) -> Result<(), Error> {
        for id in self.shelf_ids()? {
            if Some(id) == keep {
                continue;
            }
            let Some(on_disk) = self.read_shelf_lenient(id)? else {
                continue;
            };
            let mut shelf = on_disk.shelf;
            if !shelf.remove_volume(volume) {
                continue;
            }
            // 指向已删册的编号在读的时候本来就会丢，写回时顺手清掉。
            shelf.volumes.retain(|held| self.volume_exists(*held));
            if shelf.volumes.is_empty() {
                self.delete_shelf(id)?;
            } else {
                self.save_shelf_at(id, &shelf, stamp)?;
            }
        }
        Ok(())
    }

    /// 下一个可用的册编号，从 1 起。0 留给 Empty book，永不分配；删过的号也不复用。
    pub fn next_volume(&self) -> Result<u64, Error> {
        let max = self
            .volume_ids()?
            .into_iter()
            .chain(trashed_ids(self.root(), KIND))
            .max();
        // 只有 Empty book 时最大号是 0，加一正好从 1 起；没有册也给 1。
        Ok(max.map_or(1, |id| id + 1).max(1))
    }

    /// 一页属于哪本册。页文件不在给 None；页在盘上却没记在任何册文件里就是 Empty book。
    /// 扫一遍册目录，页文件里不记反向字段。
    pub fn volume_of_page(&self, page: u64) -> Result<Option<u64>, Error> {
        // 页没了，册里残留的编号就不算归属：和 load 时丢掉悬挂编号同一个口径。
        if !self.page_exists(page) {
            return Ok(None);
        }
        Ok(self
            .volumes_holding(page)?
            .into_iter()
            .map(|(id, _)| id)
            .min()
            // 没有归属记录，或者那本册文件坏了读不出来：都算 Empty book 的页。
            .or(Some(EMPTY_BOOK_ID)))
    }

    /// 把一页放进目标册的第 `at` 位。它原来在别的册里时先从那本册里摘掉。
    /// 返回它原来在哪本册；原来就在目标册里给 None。
    ///
    /// 页在盘上却没记在任何册文件里算 Empty book 的页，所以会返回 Some(0)。
    /// 跨多个文件写，不是事务：中途失败最多让这一页暂时没有归属记录，
    /// 页文件本身不动，列表里它仍然出现在 Empty book 下。
    pub fn assign_page(&self, volume_id: u64, page: u64, at: usize) -> Result<Option<u64>, Error> {
        let stamp = now_seconds();
        let previous = self.volume_of_page(page)?;
        let mut target = self.load_volume(volume_id)?;
        let removed = self.detach_page(page, Some(volume_id), stamp)?;
        target.insert_page(at, page);
        self.save_volume_at(volume_id, &target, stamp)?;
        Ok(removed.or(previous.filter(|id| *id != volume_id)))
    }

    /// 把一页放回 Empty book 的末尾。返回它原来在哪本册。
    ///
    /// 页原来就在 Empty book 里返回 Some(0)，页文件已经不在了返回 None。
    /// 坏数据里一页可能同时躺在好几本册里，这里按"摘掉其余、留在 Empty book"处理。
    pub fn unassign_page(&self, page: u64) -> Result<Option<u64>, Error> {
        let stamp = now_seconds();
        let previous = self.volume_of_page(page)?;
        // 先摘掉除 Empty book 之外所有册里的引用。
        let removed = self.detach_page(page, Some(EMPTY_BOOK_ID), stamp)?;
        // 页文件还在才写回编号：指向已删页的编号不进册。
        if self.page_exists(page) {
            let mut empty = self.load_volume(EMPTY_BOOK_ID)?;
            if empty.insert_page(usize::MAX, page) {
                self.save_volume_at(EMPTY_BOOK_ID, &empty, stamp)?;
            }
        }
        Ok(previous.or(removed))
    }

    /// 把一页从一本册挪到另一本册的第 `at` 位：一次完成摘除与插入。
    ///
    /// 页其实不在 `from_volume` 里也不报错：归属以册文件为准，先按实际归属摘掉再插。
    pub fn move_page(
        &self,
        from_volume: u64,
        to_volume: u64,
        page: u64,
        at: usize,
    ) -> Result<(), Error> {
        if from_volume == to_volume {
            // 同一本册里挪位置：摘掉再按新下标插回，时间戳更新一次。
            let stamp = now_seconds();
            let mut volume = self.load_volume(to_volume)?;
            if volume.remove_page(page) {
                volume.insert_page(at, page);
                self.save_volume_at(to_volume, &volume, stamp)?;
            }
            return Ok(());
        }
        self.assign_page(to_volume, page, at)?;
        Ok(())
    }

    /// 册内重排：把第 `from` 页挪到第 `to` 位。下标越界夹到边界。
    pub fn move_page_in_volume(&self, volume_id: u64, from: usize, to: usize) -> Result<(), Error> {
        let mut volume = self.load_volume(volume_id)?;
        if volume.move_page(from, to) {
            self.save_volume_at(volume_id, &volume, now_seconds())?;
        }
        Ok(())
    }

    /// 一本册里的页，页序取自册文件，指向已删页的编号不算。
    ///
    /// 其它册列的就是全部；Empty book 额外收下盘上的孤儿页——真实存在、却不在任何册
    /// 文件里的页。册文件损坏时它名下的编号读不出来，那些页就落到这里，
    /// 于是不会因为一个坏文件从界面上消失。
    pub fn pages_in_volume(&self, id: u64) -> Result<Vec<u64>, Error> {
        let listed = if id == EMPTY_BOOK_ID {
            self.read_volume_lenient(id)?
                .map_or_else(Vec::new, |on_disk| on_disk.volume.pages)
        } else {
            self.load_volume(id)?.pages
        };
        let mut seen = BTreeSet::new();
        let mut pages = Vec::new();
        for page in listed {
            if self.page_exists(page) && seen.insert(page) {
                pages.push(page);
            }
        }
        if id == EMPTY_BOOK_ID {
            for orphan in self.orphan_pages()? {
                if seen.insert(orphan) {
                    pages.push(orphan);
                }
            }
        }
        Ok(pages)
    }

    /// 盘上存在、却不在任何册文件里的页，升序。册文件坏掉时它的页就在这里。
    fn orphan_pages(&self) -> Result<Vec<u64>, Error> {
        let mut assigned = BTreeSet::new();
        for id in self.volume_ids()? {
            let Some(on_disk) = self.read_volume_lenient(id)? else {
                continue;
            };
            assigned.extend(on_disk.volume.pages);
        }
        Ok(self
            .board_ids()?
            .into_iter()
            .filter(|id| !assigned.contains(id))
            .collect())
    }

    /// 形如 `volumes/<id>.volume` 的路径。
    pub(crate) fn volume_path(&self, id: u64) -> PathBuf {
        self.root().join(VOLUMES_DIR).join(format!("{id}.{KIND}"))
    }

    pub(crate) fn volume_ids(&self) -> Result<Vec<u64>, Error> {
        ids_in(&self.root().join(VOLUMES_DIR), KIND)
    }

    /// 读册文件。文件不在给 None；内容读不懂是错误。
    pub(crate) fn read_volume(&self, id: u64) -> Result<Option<VolumeOnDisk>, Error> {
        let bytes = match fs::read(self.volume_path(id)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let value = cbor::decode(&bytes)?;
        Ok(Some(volume_from(&value)?))
    }

    /// 列表与归属查询用的宽松版：单个册读不懂只跳过它自己；
    /// 更高版本的格式仍然要报错，否则用户会以为文件丢了。
    pub(crate) fn read_volume_lenient(&self, id: u64) -> Result<Option<VolumeOnDisk>, Error> {
        match self.read_volume(id) {
            Ok(volume) => Ok(volume),
            Err(error @ Error::NewerFormat { .. }) => Err(error),
            Err(_) => Ok(None),
        }
    }

    /// 所有含这一页的册，按编号升序。正常最多一本；坏数据里可能不止，摘除时要全部处理。
    fn volumes_holding(&self, page: u64) -> Result<Vec<(u64, Volume)>, Error> {
        let mut found = Vec::new();
        for id in self.volume_ids()? {
            let Some(on_disk) = self.read_volume_lenient(id)? else {
                continue;
            };
            if on_disk.volume.pages.contains(&page) {
                found.push((id, on_disk.volume));
            }
        }
        Ok(found)
    }

    /// 从所有册里摘掉这一页，跳过 `keep` 那本。返回第一本被改动的册。
    pub(crate) fn detach_page(
        &self,
        page: u64,
        keep: Option<u64>,
        stamp: i64,
    ) -> Result<Option<u64>, Error> {
        let mut removed_from = None;
        for (id, mut volume) in self.volumes_holding(page)? {
            if Some(id) == keep {
                continue;
            }
            volume.remove_page(page);
            self.save_volume_at(id, &volume, stamp)?;
            if removed_from.is_none() {
                removed_from = Some(id);
            }
        }
        Ok(removed_from)
    }

    /// 页文件还在不在。册里指向已删页的编号按不存在处理。
    fn page_exists(&self, page: u64) -> bool {
        self.board_path(page).is_file()
    }
}
