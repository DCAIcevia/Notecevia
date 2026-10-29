//! 册与书架：有序的编号表，只存引用。
//!
//! 册是一本书：一个名字加一串页编号；书架是一组册：一个名字加一串册编号。
//! 页永远属于一个册：编号 0 的 Empty book 是默认册，始终存在、不可删除，也可以是空册。
//! 一个页最多属于一个册——归属由册文件的编号表说了算，这条约束跨多个册，
//! 由 store 的写入接口保证；这里只管单个册内编号不重复。
//! 册与书架之间只有引用：同一本册可以出现在多个书架上，删书架不删册。
//!
//! 不碰文件、不碰时钟、不碰随机数。

/// 默认册的编号。Empty book 始终存在、不可删除，0 永不分配。
pub const EMPTY_BOOK_ID: u64 = 0;

/// 默认册的标题。工作区打开时用它建出 `volumes/0.volume`。
pub const EMPTY_BOOK_TITLE: &str = "Empty book";

/// 一层书架至少要有这么多册。少于这个数不成架：书架由两册合成。
pub const MIN_SHELF_VOLUMES: usize = 2;

/// 册：一本书。`pages` 是有序的页编号，也是这些页的归属。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub title: String,
    pub pages: Vec<u64>,
}

impl Volume {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            pages: Vec::new(),
        }
    }

    /// 默认册 Empty book：编号 [EMPTY_BOOK_ID]，标题 [EMPTY_BOOK_TITLE]，一开始没有页。
    pub fn empty_book() -> Self {
        Self::new(EMPTY_BOOK_TITLE)
    }

    /// 把一页插到第 `index` 位。下标越界就夹到末尾，编号已经在册里就不动。
    /// 返回有没有真的插进去。
    pub fn insert_page(&mut self, index: usize, page: u64) -> bool {
        insert_unique(&mut self.pages, index, page)
    }

    /// 把一页从册里摘掉。不在这本册里就是空操作；返回有没有摘掉。
    pub fn remove_page(&mut self, page: u64) -> bool {
        remove_id(&mut self.pages, page)
    }

    /// 把第 `from` 页挪到第 `to` 位。下标越界夹到边界，挪回原位算没动。
    /// 返回顺序有没有真的变。
    pub fn move_page(&mut self, from: usize, to: usize) -> bool {
        move_id(&mut self.pages, from, to)
    }
}

/// 书架：一组册。`volumes` 是有序的册编号，同一本册可以出现在多个书架上。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shelf {
    pub title: String,
    pub volumes: Vec<u64>,
}

impl Shelf {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            volumes: Vec::new(),
        }
    }

    /// 把一本册插到第 `index` 位。下标越界就夹到末尾，编号已经在架子上就不动。
    /// 返回有没有真的插进去。
    pub fn insert_volume(&mut self, index: usize, volume: u64) -> bool {
        insert_unique(&mut self.volumes, index, volume)
    }

    /// 把一本册从架子上拿掉。不在这层架子上就是空操作；返回有没有拿掉。
    pub fn remove_volume(&mut self, volume: u64) -> bool {
        remove_id(&mut self.volumes, volume)
    }

    /// 把第 `from` 本册挪到第 `to` 位。下标越界夹到边界，挪回原位算没动。
    /// 返回顺序有没有真的变。
    pub fn move_volume(&mut self, from: usize, to: usize) -> bool {
        move_id(&mut self.volumes, from, to)
    }
}

/// 编号表共用的插入：不重复，越界夹到末尾。
fn insert_unique(ids: &mut Vec<u64>, index: usize, id: u64) -> bool {
    if ids.contains(&id) {
        return false;
    }
    ids.insert(index.min(ids.len()), id);
    true
}

/// 编号表共用的移除：不在表里就是空操作。
fn remove_id(ids: &mut Vec<u64>, id: u64) -> bool {
    match ids.iter().position(|&item| item == id) {
        Some(index) => {
            ids.remove(index);
            true
        }
        None => false,
    }
}

/// 编号表共用的移动：空表或原位不动，其余把下标夹到边界后再挪。
fn move_id(ids: &mut Vec<u64>, from: usize, to: usize) -> bool {
    if ids.is_empty() {
        return false;
    }
    let from = from.min(ids.len() - 1);
    let to = to.min(ids.len() - 1);
    if from == to {
        return false;
    }
    let id = ids.remove(from);
    ids.insert(to, id);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume_of(pages: &[u64]) -> Volume {
        let mut volume = Volume::new("册");
        for (index, &page) in pages.iter().enumerate() {
            volume.insert_page(index, page);
        }
        volume
    }

    #[test]
    fn the_empty_book_uses_the_default_id_and_title() {
        let empty = Volume::empty_book();
        assert_eq!(empty.title, EMPTY_BOOK_TITLE);
        assert_eq!(empty.title, "Empty book");
        assert_eq!(EMPTY_BOOK_ID, 0);
        assert!(empty.pages.is_empty());
    }

    #[test]
    fn a_new_volume_has_no_pages() {
        let volume = Volume::new("合集");
        assert_eq!(volume.title, "合集");
        assert!(volume.pages.is_empty());
    }

    #[test]
    fn inserting_at_an_index_keeps_the_order() {
        let mut volume = volume_of(&[1, 3]);
        assert!(volume.insert_page(1, 2));
        assert_eq!(volume.pages, vec![1, 2, 3]);
        assert!(volume.insert_page(0, 9));
        assert_eq!(volume.pages, vec![9, 1, 2, 3]);
    }

    #[test]
    fn inserting_past_the_end_lands_at_the_end() {
        let mut volume = volume_of(&[1, 2]);
        assert!(volume.insert_page(99, 3));
        assert_eq!(volume.pages, vec![1, 2, 3]);
    }

    #[test]
    fn the_same_page_is_not_inserted_twice() {
        let mut volume = volume_of(&[1, 2]);
        assert!(!volume.insert_page(0, 2));
        assert!(!volume.insert_page(9, 1));
        assert_eq!(volume.pages, vec![1, 2]);
    }

    #[test]
    fn removing_a_page_that_is_not_there_does_nothing() {
        let mut volume = volume_of(&[1, 2, 3]);
        assert!(volume.remove_page(2));
        assert_eq!(volume.pages, vec![1, 3]);
        assert!(!volume.remove_page(2));
        assert_eq!(volume.pages, vec![1, 3]);
    }

    #[test]
    fn moving_a_page_reorders_and_clamps() {
        let mut volume = volume_of(&[1, 2, 3]);
        assert!(volume.move_page(0, 2));
        assert_eq!(volume.pages, vec![2, 3, 1]);
        // 越界的 to 夹到最后一个位置：第一页挪到末尾。
        assert!(volume.move_page(0, 99));
        assert_eq!(volume.pages, vec![3, 1, 2]);
        // 越界的 from 夹到最后一页：最后一页挪到最前。
        assert!(volume.move_page(99, 0));
        assert_eq!(volume.pages, vec![2, 3, 1]);
        // 原位不算动，空转返回 false。
        assert!(!volume.move_page(0, 0));
        assert!(!volume.move_page(99, 99));
        assert_eq!(volume.pages, vec![2, 3, 1]);
    }

    #[test]
    fn moving_page_in_an_empty_volume_does_nothing() {
        let mut volume = Volume::new("空册");
        assert!(!volume.move_page(0, 1));
        assert!(!volume.move_page(5, 0));
        assert!(volume.pages.is_empty());
    }

    #[test]
    fn a_shelf_repeats_the_same_rules() {
        let mut shelf = Shelf::new("书架");
        assert!(shelf.volumes.is_empty());
        assert!(shelf.insert_volume(0, 3));
        assert!(shelf.insert_volume(1, 1));
        assert!(shelf.insert_volume(99, 2));
        assert_eq!(shelf.volumes, vec![3, 1, 2]);
        // 同一本册在架子上只留一份。
        assert!(!shelf.insert_volume(0, 1));
        assert_eq!(shelf.volumes, vec![3, 1, 2]);
        assert!(shelf.move_volume(2, 0));
        assert_eq!(shelf.volumes, vec![2, 3, 1]);
        assert!(shelf.remove_volume(3));
        assert_eq!(shelf.volumes, vec![2, 1]);
        assert!(!shelf.remove_volume(3));
        assert_eq!(shelf.volumes, vec![2, 1]);
    }

    #[test]
    fn two_volumes_may_hold_the_same_page_in_this_layer() {
        // 独占是跨册的约束，由 store 的写入接口保证；这里只保证单个册内不重复。
        let mut first = Volume::new("甲");
        let mut second = Volume::new("乙");
        assert!(first.insert_page(0, 7));
        assert!(second.insert_page(0, 7));
        assert_eq!(first.pages, vec![7]);
        assert_eq!(second.pages, vec![7]);
    }
}
