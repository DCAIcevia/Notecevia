//! 册与书架：存读往返、列表顺序、回收站、归属与编号不复用。
//!
//! 页永远属于一个册：0 号 Empty book 常驻、删不掉，兜住没有归属记录的页。
//! 这里重点测：一页只能在 Empty book 与一本册之间二选一、删册连页一起删、
//! 孤儿页落进 Empty book、编号不复用、Empty book 固定在列表最后。

mod support;

use std::fs;

use notecevia_core::board::Document;
use notecevia_core::volume::{EMPTY_BOOK_ID, EMPTY_BOOK_TITLE, Shelf, Volume};
use notecevia_store::{Error, ShelfInfo, VolumeInfo, Workspace, cbor};

use support::{TempWorkspace, files_with_extension};

/// 写一页。册里的编号指向真实页文件时才不会被当成悬挂编号丢掉。
fn add_page(workspace: &Workspace, id: u64) {
    workspace.save(id, &Document::new("一页")).expect("写一页");
}

/// 建一本册，页也一起建出来。
fn save_volume_with(workspace: &Workspace, id: u64, title: &str, pages: &[u64]) {
    let mut volume = Volume::new(title);
    for (index, &page) in pages.iter().enumerate() {
        add_page(workspace, page);
        volume.insert_page(index, page);
    }
    workspace.save_volume(id, &volume).expect("存册");
}

fn volume_ids(infos: &[VolumeInfo]) -> Vec<u64> {
    infos.iter().map(|info| info.id).collect()
}

#[test]
fn opening_a_workspace_creates_the_volume_and_shelf_directories() {
    let temp = TempWorkspace::new("volumes-dirs");
    assert!(temp.path("volumes").is_dir());
    assert!(temp.path("shelves").is_dir());
}

#[test]
fn opening_a_new_workspace_creates_the_empty_book() {
    let temp = TempWorkspace::new("empty-book");
    assert!(temp.path("volumes/0.volume").is_file());

    let infos = temp.workspace.list_volumes().expect("列册");
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].id, EMPTY_BOOK_ID);
    assert_eq!(infos[0].title, EMPTY_BOOK_TITLE);
    assert_eq!(infos[0].page_count, 0);

    // 默认册不许删，文件也不动。
    assert!(matches!(
        temp.workspace.delete_volume(EMPTY_BOOK_ID),
        Err(Error::CannotDeleteEmptyBook)
    ));
    assert!(temp.path("volumes/0.volume").is_file());

    // 文件被外部删掉，重开工作区就补回来。
    fs::remove_file(temp.path("volumes/0.volume")).expect("删掉默认册文件");
    let reopened = Workspace::open(&temp.root).expect("重开工作区");
    assert!(temp.path("volumes/0.volume").is_file());
    assert_eq!(reopened.list_volumes().expect("列册")[0].id, EMPTY_BOOK_ID);
}

#[test]
fn a_volume_roundtrips_through_disk() {
    let temp = TempWorkspace::new("volume-roundtrip");
    let mut volume = Volume::new("合集");
    for (index, page) in [3_u64, 1, 2].into_iter().enumerate() {
        add_page(&temp.workspace, page);
        volume.insert_page(index, page);
    }
    temp.workspace.save_volume(5, &volume).expect("存册");
    assert!(temp.path("volumes/5.volume").is_file());
    assert_eq!(temp.workspace.load_volume(5).expect("读册"), volume);
}

#[test]
fn volumes_are_listed_newest_first_and_the_empty_book_is_last() {
    let temp = TempWorkspace::new("volume-order");
    temp.workspace
        .save_volume_at(1, &Volume::new("旧"), 100)
        .expect("存册");
    temp.workspace
        .save_volume_at(2, &Volume::new("新"), 300)
        .expect("存册");
    temp.workspace
        .save_volume_at(3, &Volume::new("中"), 200)
        .expect("存册");
    // 同一秒的两本按编号倒序，列表才稳定。
    temp.workspace
        .save_volume_at(4, &Volume::new("同秒甲"), 200)
        .expect("存册");

    let infos = temp.workspace.list_volumes().expect("列册");
    assert_eq!(volume_ids(&infos), vec![2, 4, 3, 1, EMPTY_BOOK_ID]);
    assert_eq!(infos[0].title, "新");
    assert_eq!(infos[0].updated_at, 300);
    assert_eq!(infos[1].page_count, 0);
    // Empty book 不管修改时间都排在最后一个。
    assert_eq!(infos.last().expect("最后一行").title, EMPTY_BOOK_TITLE);
}

#[test]
fn deleting_a_volume_takes_its_pages_and_shelf_entries() {
    let temp = TempWorkspace::new("volume-delete-cascade");
    save_volume_with(&temp.workspace, 10, "删我", &[1, 2]);
    save_volume_with(&temp.workspace, 5, "留下", &[3]);
    let mut shelf = Shelf::new("书架");
    shelf.insert_volume(0, 5);
    shelf.insert_volume(1, 10);
    temp.workspace.save_shelf(1, &shelf).expect("存书架");

    temp.workspace.delete_volume(10).expect("删册");

    // 册文件进了回收站。
    assert!(!temp.path("volumes/10.volume").exists());
    let trashed_volumes = files_with_extension(&temp.path(".trash"), "volume");
    assert_eq!(trashed_volumes.len(), 1);
    // 它的页也跟着进回收站：页文件不在原位了。
    for page in [1_u64, 2] {
        assert!(!temp.path(&format!("boards/{page}.board")).exists());
    }
    assert_eq!(files_with_extension(&temp.path(".trash"), "board").len(), 2);
    // 书架还在，只把删掉的册摘下来；别的册与它的页不动。
    assert!(temp.path("shelves/1.shelf").is_file());
    assert_eq!(
        temp.workspace.load_shelf(1).expect("读书架").volumes,
        vec![5]
    );
    assert_eq!(
        temp.workspace.pages_in_volume(5).expect("留下的册"),
        vec![3]
    );
    // 页文件已经不在盘上，Empty book 里没有它们。
    assert!(
        temp.workspace
            .pages_in_volume(EMPTY_BOOK_ID)
            .expect("Empty book")
            .is_empty()
    );
    assert!(matches!(temp.workspace.load_volume(10), Err(Error::Io(_))));
    assert_eq!(
        temp.workspace.list_volumes().expect("列册")[0].id,
        5,
        "只剩留下那本与 Empty book"
    );
}

#[test]
fn a_broken_volume_file_is_skipped_but_keeps_its_number() {
    let temp = TempWorkspace::new("volume-broken");
    save_volume_with(&temp.workspace, 1, "甲", &[1]);
    save_volume_with(&temp.workspace, 2, "乙", &[2]);
    // 0xff 是 CBOR 的保留位，解码直接报错，和"读不懂的坏文件"是同一类。
    fs::write(temp.path("volumes/7.volume"), [0xff, 0xff]).expect("写坏文件");

    let infos = temp.workspace.list_volumes().expect("坏文件不影响列表");
    assert_eq!(volume_ids(&infos), vec![2, 1, EMPTY_BOOK_ID]);
    // 坏文件的编号还能被指出来：load 报具体错误，编号也不被复用。
    assert!(matches!(temp.workspace.load_volume(7), Err(Error::Cbor(_))));
    assert_eq!(temp.workspace.next_volume().expect("下一个册号"), 8);
}

#[test]
fn a_newer_volume_format_is_reported_instead_of_skipped() {
    let temp = TempWorkspace::new("volume-newer");
    let value = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(99)),
        ("kind", cbor::Value::text("volume")),
        ("title", cbor::Value::text("未来写的")),
        ("pages", cbor::Value::Array(Vec::new())),
        ("updated_at", cbor::Value::Integer(1)),
    ]);
    fs::write(temp.path("volumes/1.volume"), cbor::encode(&value)).expect("写文件");

    assert!(matches!(
        temp.workspace.list_volumes(),
        Err(Error::NewerFormat { found: 99, .. })
    ));
    // 更高版本的文件不允许被覆盖。
    assert!(matches!(
        temp.workspace.save_volume(1, &Volume::new("我的")),
        Err(Error::NewerFormat { found: 99, .. })
    ));
}

#[test]
fn unknown_volume_fields_survive_a_rewrite() {
    let temp = TempWorkspace::new("volume-extras");
    let value = cbor::Value::map(vec![
        ("format", cbor::Value::Integer(1)),
        ("kind", cbor::Value::text("volume")),
        ("title", cbor::Value::text("甲")),
        ("pages", cbor::Value::Array(Vec::new())),
        ("updated_at", cbor::Value::Integer(5)),
        ("future", cbor::Value::text("留着")),
    ]);
    fs::write(temp.path("volumes/1.volume"), cbor::encode(&value)).expect("写文件");

    temp.workspace
        .save_volume_at(1, &Volume::new("乙"), 9)
        .expect("重写旧文件");
    let bytes = fs::read(temp.path("volumes/1.volume")).expect("读回");
    let value = cbor::decode(&bytes).expect("解码");
    assert_eq!(
        value.get("future").and_then(cbor::Value::as_text),
        Some("留着")
    );
    assert_eq!(temp.workspace.load_volume(1).expect("读册").title, "乙");
}

#[test]
fn next_volume_skips_zero_and_never_reuses() {
    let temp = TempWorkspace::new("volume-ids");
    // 0 是 Empty book 的编号，永不分配。
    assert_eq!(temp.workspace.next_volume().expect("下一个册号"), 1);
    save_volume_with(&temp.workspace, 1, "甲", &[1]);
    save_volume_with(&temp.workspace, 2, "乙", &[2]);
    assert_eq!(temp.workspace.next_volume().expect("下一个册号"), 3);

    temp.workspace.delete_volume(2).expect("删册");
    // 回收站里的册文件夹也算进最大号，不会被新册顶上。
    assert_eq!(temp.workspace.next_volume().expect("下一个册号"), 3);

    save_volume_with(&temp.workspace, 3, "丙", &[3]);
    assert_eq!(temp.workspace.next_volume().expect("下一个册号"), 4);
    temp.workspace.delete_volume(1).expect("删册");
    temp.workspace.delete_volume(3).expect("删册");
    assert_eq!(temp.workspace.next_volume().expect("下一个册号"), 4);
}

#[test]
fn a_page_lives_in_at_most_one_volume() {
    let temp = TempWorkspace::new("volume-exclusive");
    add_page(&temp.workspace, 1);
    add_page(&temp.workspace, 2);
    // 刚写下的页没有归属记录，算 Empty book 的页。
    assert_eq!(
        temp.workspace
            .pages_in_volume(EMPTY_BOOK_ID)
            .expect("Empty book"),
        vec![1, 2]
    );
    temp.workspace
        .save_volume(10, &Volume::new("甲"))
        .expect("存册");
    temp.workspace
        .save_volume(11, &Volume::new("乙"))
        .expect("存册");

    assert_eq!(
        temp.workspace.assign_page(10, 1, 0).expect("入册"),
        Some(EMPTY_BOOK_ID),
        "原来在 Empty book 里"
    );
    // 一页只有一本册：入册之后 Empty book 里就没有它了。
    assert_eq!(
        temp.workspace
            .pages_in_volume(EMPTY_BOOK_ID)
            .expect("Empty book"),
        vec![2]
    );
    assert_eq!(temp.workspace.pages_in_volume(10).expect("甲"), vec![1]);

    // 再入另一本册：旧册里必须先摘掉，并告诉上层摘的是哪本。
    assert_eq!(
        temp.workspace.assign_page(11, 1, 0).expect("入册"),
        Some(10)
    );
    assert!(temp.workspace.pages_in_volume(10).expect("甲").is_empty());
    assert_eq!(temp.workspace.pages_in_volume(11).expect("乙"), vec![1]);
    assert_eq!(temp.workspace.volume_of_page(1).expect("查归属"), Some(11));
}

#[test]
fn assigning_keeps_the_requested_order_and_clamps() {
    let temp = TempWorkspace::new("volume-assign-order");
    for page in 1..=4 {
        add_page(&temp.workspace, page);
    }
    temp.workspace
        .save_volume(20, &Volume::new("册"))
        .expect("存册");

    temp.workspace.assign_page(20, 1, 0).expect("入册");
    temp.workspace.assign_page(20, 3, 1).expect("入册");
    temp.workspace.assign_page(20, 2, 1).expect("入册");
    assert_eq!(
        temp.workspace.load_volume(20).expect("读册").pages,
        vec![1, 2, 3]
    );
    // 越界的下标夹到末尾。
    temp.workspace.assign_page(20, 4, 99).expect("入册");
    assert_eq!(
        temp.workspace.pages_in_volume(20).expect("册里"),
        vec![1, 2, 3, 4]
    );
    // 四页都入册了，Empty book 空了。
    assert!(
        temp.workspace
            .pages_in_volume(EMPTY_BOOK_ID)
            .expect("Empty book")
            .is_empty()
    );
}

#[test]
fn unassigning_puts_the_page_back_in_the_empty_book() {
    let temp = TempWorkspace::new("volume-unassign");
    add_page(&temp.workspace, 1);
    temp.workspace
        .save_volume(10, &Volume::new("甲"))
        .expect("存册");
    temp.workspace.assign_page(10, 1, 0).expect("入册");

    assert_eq!(temp.workspace.unassign_page(1).expect("放回"), Some(10));
    assert!(
        temp.workspace
            .load_volume(10)
            .expect("读册")
            .pages
            .is_empty()
    );
    assert_eq!(
        temp.workspace
            .pages_in_volume(EMPTY_BOOK_ID)
            .expect("Empty book"),
        vec![1]
    );
    assert_eq!(temp.workspace.volume_of_page(1).expect("查归属"), Some(0));

    // 已经在 Empty book 里：原册就是它自己。
    assert_eq!(
        temp.workspace.unassign_page(1).expect("再放回"),
        Some(EMPTY_BOOK_ID)
    );
    // 页文件不在了，没有可放回的页。
    fs::remove_file(temp.path("boards/1.board")).expect("删页文件");
    assert_eq!(temp.workspace.unassign_page(1).expect("页不在"), None);
}

#[test]
fn moving_a_page_between_volumes_leaves_the_source() {
    let temp = TempWorkspace::new("volume-move");
    add_page(&temp.workspace, 1);
    add_page(&temp.workspace, 2);
    save_volume_with(&temp.workspace, 10, "甲", &[1, 2]);
    temp.workspace
        .save_volume(11, &Volume::new("乙"))
        .expect("存册");

    temp.workspace.move_page(10, 11, 2, 0).expect("挪页");
    assert_eq!(temp.workspace.load_volume(10).expect("读册").pages, vec![1]);
    assert_eq!(temp.workspace.load_volume(11).expect("读册").pages, vec![2]);
}

#[test]
fn moving_a_page_inside_a_volume_reorders_it() {
    let temp = TempWorkspace::new("volume-reorder");
    save_volume_with(&temp.workspace, 10, "甲", &[1, 2, 3]);
    temp.workspace
        .move_page_in_volume(10, 0, 2)
        .expect("册内重排");
    assert_eq!(
        temp.workspace.load_volume(10).expect("读册").pages,
        vec![2, 3, 1]
    );
}

#[test]
fn deleting_a_board_removes_it_from_its_volume() {
    let temp = TempWorkspace::new("volume-delete-board");
    save_volume_with(&temp.workspace, 10, "甲", &[1, 2]);

    temp.workspace.delete_board(1).expect("删页");
    assert_eq!(temp.workspace.load_volume(10).expect("读册").pages, vec![2]);
    assert_eq!(temp.workspace.volume_of_page(1).expect("查归属"), None);
    assert_eq!(temp.workspace.pages_in_volume(10).expect("册里"), vec![2]);
}

#[test]
fn page_ids_without_a_file_are_dropped_on_load() {
    let temp = TempWorkspace::new("volume-dangling");
    save_volume_with(&temp.workspace, 10, "甲", &[1, 2]);
    // 外部手删页文件，册里就留下悬挂编号。
    fs::remove_file(temp.path("boards/1.board")).expect("删页文件");

    assert_eq!(temp.workspace.load_volume(10).expect("读册").pages, vec![2]);
    let infos = temp.workspace.list_volumes().expect("列册");
    assert_eq!(infos[0].page_count, 1);
    assert_eq!(temp.workspace.volume_of_page(1).expect("查归属"), None);
    // 页 2 还在册里；页 1 的文件没了，哪本册里都不该冒出它。
    assert_eq!(temp.workspace.pages_in_volume(10).expect("册里"), vec![2]);
    assert!(
        temp.workspace
            .pages_in_volume(EMPTY_BOOK_ID)
            .expect("Empty book")
            .is_empty()
    );
}

#[test]
fn orphan_pages_show_up_in_the_empty_book() {
    let temp = TempWorkspace::new("volume-orphans");
    save_volume_with(&temp.workspace, 10, "甲", &[1]);
    // 盘上真实存在、没记在任何册文件里的页。
    add_page(&temp.workspace, 2);
    // 11 号册先记下页 3，再把文件弄坏：编号读不出来，页就成了孤儿页。
    save_volume_with(&temp.workspace, 11, "坏了", &[3]);
    fs::write(temp.path("volumes/11.volume"), [0xff, 0xff]).expect("写坏文件");

    assert_eq!(
        temp.workspace
            .pages_in_volume(EMPTY_BOOK_ID)
            .expect("Empty book"),
        vec![2, 3]
    );
    assert_eq!(temp.workspace.pages_in_volume(10).expect("甲"), vec![1]);
    assert_eq!(
        temp.workspace.volume_of_page(3).expect("查归属"),
        Some(EMPTY_BOOK_ID)
    );

    let infos = temp.workspace.list_volumes().expect("列册");
    assert_eq!(volume_ids(&infos), vec![10, EMPTY_BOOK_ID]);
    let empty = infos.last().expect("最后一行");
    assert_eq!(empty.title, EMPTY_BOOK_TITLE);
    assert_eq!(empty.page_count, 2);
}

#[test]
fn a_shelf_roundtrips_and_lists_its_volumes() {
    let temp = TempWorkspace::new("shelf-roundtrip");
    save_volume_with(&temp.workspace, 5, "甲", &[1]);
    save_volume_with(&temp.workspace, 6, "乙", &[2]);
    let mut shelf = Shelf::new("书架");
    shelf.insert_volume(0, 6);
    shelf.insert_volume(1, 5);
    temp.workspace
        .save_shelf_at(2, &shelf, 1_234)
        .expect("存书架");
    assert_eq!(temp.workspace.load_shelf(2).expect("读书架"), shelf);

    // 同一本册可以出现在多个书架上：架子只是引用。
    let mut other = Shelf::new("另一个书架");
    other.insert_volume(0, 6);
    temp.workspace
        .save_shelf_at(3, &other, 1_000)
        .expect("存书架");

    let infos = temp.workspace.list_shelves().expect("列书架");
    assert_eq!(
        infos,
        vec![
            ShelfInfo {
                id: 2,
                title: "书架".to_owned(),
                volume_count: 2,
                updated_at: 1_234,
            },
            ShelfInfo {
                id: 3,
                title: "另一个书架".to_owned(),
                volume_count: 1,
                updated_at: 1_000,
            },
        ]
    );
}

#[test]
fn deleting_a_shelf_moves_the_file_to_trash_and_keeps_volumes_and_pages() {
    let temp = TempWorkspace::new("shelf-trash");
    save_volume_with(&temp.workspace, 5, "甲", &[1]);
    let mut shelf = Shelf::new("书架");
    shelf.insert_volume(0, 5);
    temp.workspace.save_shelf(1, &shelf).expect("存书架");

    temp.workspace.delete_shelf(1).expect("删书架");
    assert!(!temp.path("shelves/1.shelf").exists());
    let trashed = files_with_extension(&temp.path(".trash"), "shelf");
    assert_eq!(trashed.len(), 1);
    assert!(matches!(temp.workspace.load_shelf(1), Err(Error::Io(_))));
    // 册与页都不动。
    assert_eq!(temp.workspace.load_volume(5).expect("读册").pages, vec![1]);
    assert_eq!(temp.workspace.pages_in_volume(5).expect("册里"), vec![1]);
    assert!(temp.path("boards/1.board").is_file());
}

#[test]
fn shelf_entries_for_deleted_volumes_are_dropped() {
    let temp = TempWorkspace::new("shelf-dangling");
    save_volume_with(&temp.workspace, 5, "甲", &[1]);
    save_volume_with(&temp.workspace, 6, "乙", &[2]);
    let mut shelf = Shelf::new("书架");
    shelf.insert_volume(0, 6);
    shelf.insert_volume(1, 5);
    temp.workspace.save_shelf(1, &shelf).expect("存书架");

    temp.workspace.delete_volume(6).expect("删册");
    assert_eq!(
        temp.workspace.load_shelf(1).expect("读书架").volumes,
        vec![5]
    );
    let infos = temp.workspace.list_shelves().expect("列书架");
    assert_eq!(infos[0].volume_count, 1);
}

#[test]
fn shelf_ids_are_not_reused_after_delete() {
    let temp = TempWorkspace::new("shelf-ids");
    temp.workspace
        .save_shelf(1, &Shelf::new("甲"))
        .expect("存书架");
    temp.workspace
        .save_shelf(2, &Shelf::new("乙"))
        .expect("存书架");
    assert_eq!(temp.workspace.next_shelf().expect("下一个书架号"), 3);

    temp.workspace.delete_shelf(2).expect("删书架");
    assert_eq!(temp.workspace.next_shelf().expect("下一个书架号"), 3);
    temp.workspace
        .save_shelf(3, &Shelf::new("丙"))
        .expect("存书架");
    assert_eq!(temp.workspace.next_shelf().expect("下一个书架号"), 4);
}

#[test]
fn a_shelf_needs_two_volumes_to_be_formed() {
    let temp = TempWorkspace::new("shelf-minimum");
    save_volume_with(&temp.workspace, 1, "甲", &[]);
    save_volume_with(&temp.workspace, 2, "乙", &[]);

    assert!(matches!(
        temp.workspace.create_shelf_with("孤本".to_owned(), vec![1]),
        Err(Error::ShelfTooSmall)
    ));
    assert!(temp.workspace.list_shelves().expect("列架").is_empty());

    let shelf = temp
        .workspace
        .create_shelf_with("两本".to_owned(), vec![1, 2])
        .expect("合成架");
    assert_eq!(shelf.volume_count, 2);
    assert_eq!(
        temp.workspace.load_shelf(shelf.id).expect("读架").volumes,
        vec![1, 2]
    );
}

#[test]
fn shelving_a_volume_moves_it_off_its_old_shelf() {
    let temp = TempWorkspace::new("shelf-exclusive");
    save_volume_with(&temp.workspace, 1, "甲", &[]);
    save_volume_with(&temp.workspace, 2, "乙", &[]);
    save_volume_with(&temp.workspace, 3, "丙", &[]);
    let mut old = Shelf::new("旧架");
    old.insert_volume(0, 1);
    old.insert_volume(1, 2);
    temp.workspace.save_shelf(1, &old).expect("存旧架");
    let mut target = Shelf::new("新架");
    target.insert_volume(0, 3);
    temp.workspace.save_shelf(2, &target).expect("存新架");

    temp.workspace.shelve_volume(2, 2).expect("上架");

    assert_eq!(temp.workspace.load_shelf(1).expect("旧架").volumes, vec![1]);
    assert_eq!(
        temp.workspace.load_shelf(2).expect("新架").volumes,
        vec![3, 2]
    );
}

#[test]
fn unshelving_the_last_volume_sends_the_shelf_to_the_trash() {
    let temp = TempWorkspace::new("shelf-dissolve");
    save_volume_with(&temp.workspace, 1, "甲", &[]);
    save_volume_with(&temp.workspace, 2, "乙", &[]);
    let shelf = temp
        .workspace
        .create_shelf_with("散架".to_owned(), vec![1, 2])
        .expect("合成架");

    temp.workspace
        .unshelve_volume(shelf.id, 1)
        .expect("拿掉一本");
    assert_eq!(
        temp.workspace
            .load_shelf(shelf.id)
            .expect("还剩一本")
            .volumes,
        vec![2]
    );

    temp.workspace
        .unshelve_volume(shelf.id, 2)
        .expect("拿掉最后一本");
    assert!(!temp.path(&format!("shelves/{}.shelf", shelf.id)).exists());
    assert_eq!(files_with_extension(&temp.path(".trash"), "shelf").len(), 1);
}

#[test]
fn deleting_the_last_volume_takes_its_shelf() {
    let temp = TempWorkspace::new("shelf-volume-delete");
    save_volume_with(&temp.workspace, 1, "甲", &[]);
    let mut shelf = Shelf::new("只剩它");
    shelf.insert_volume(0, 1);
    temp.workspace.save_shelf(1, &shelf).expect("存架");

    temp.workspace.delete_volume(1).expect("删册");
    assert!(!temp.path("shelves/1.shelf").exists());
    assert_eq!(files_with_extension(&temp.path(".trash"), "shelf").len(), 1);
}
