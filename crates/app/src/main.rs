#![forbid(unsafe_code)]
#![deny(rustdoc::broken_intra_doc_links)]

//! 二进制入口：组装存储与界面。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui;
use notecevia_core::volume::EMPTY_BOOK_ID;
use notecevia_core::{BlockKind, Document};
use notecevia_store::{TRASH_KEEP_DAYS, TrashKind as SavedTrashKind, Workspace, days_left};
use notecevia_ui::home::{BoardItem, ExportKind, HomeAction, HomeApp, ThumbnailSource};
use notecevia_ui::layout::{Layout, LayoutSource};
use notecevia_ui::library::{Library, LibraryAction, PageRow, VolumeRow};
use notecevia_ui::media::MediaSource;
use notecevia_ui::media_panel::{MediaAction, MediaKind, MediaPanel, MediaRow};
use notecevia_ui::shell::{MenuCommand, Shell, ViewMode};
use notecevia_ui::trash::{TrashAction, TrashKind, TrashRow, TrashView};
use notecevia_ui::{BoardApp, strings, theme};
use raw_window_handle::HasWindowHandle;

/// 自动保存的间隔，秒。改动一直在就按这个节奏落盘。
const AUTOSAVE_SECONDS: f64 = 2.0;
/// 缩略图最短刷新间隔，秒。自动保存很频繁，图不必每次都重画。
const THUMBNAIL_SECONDS: f64 = 5.0;
/// 缩略图宽度，像素。
const THUMBNAIL_WIDTH: u32 = 320;
/// 首页看外部改动的间隔，秒。
const WATCH_SECONDS: f64 = 2.0;

/// 按内容哈希读媒体。界面只拿字节，路径校验留在这里。
struct DiskMedia {
    dir: PathBuf,
}

impl MediaSource for DiskMedia {
    fn media(&self, hash: &str) -> Option<Vec<u8>> {
        if !is_hash(hash) {
            return None;
        }
        std::fs::read(self.dir.join(hash)).ok()
    }

    fn media_len(&self, hash: &str) -> Option<u64> {
        if !is_hash(hash) {
            return None;
        }
        std::fs::metadata(self.dir.join(hash))
            .ok()
            .map(|it| it.len())
    }
}

/// 主题文件：放在工作区根目录的 theme.json。换配色与尺寸不用改代码。
struct FileTheme {
    path: PathBuf,
}

impl notecevia_ui::theme::ThemeSource for FileTheme {
    fn theme_text(&self) -> Option<String> {
        std::fs::read_to_string(&self.path).ok()
    }
}

/// 界面布局文件：放在工作区根目录的 ui.json。拖过的栏宽下次打开还在。
struct FileLayout {
    path: PathBuf,
}

impl LayoutSource for FileLayout {
    fn layout_text(&self) -> Option<String> {
        std::fs::read_to_string(&self.path).ok()
    }
}

/// 首页要缩略图时才去读盘，不把一万张图塞进内存。
struct DiskThumbnails {
    dir: PathBuf,
}

impl ThumbnailSource for DiskThumbnails {
    fn thumbnail(&self, board_id: u64, _updated_at: i64) -> Option<Vec<u8>> {
        std::fs::read(self.dir.join(format!("{board_id}.png"))).ok()
    }
}

/// 界面要的那几件窗口行为的实现。只把平台层包一下，不把平台类型漏给界面。
#[derive(Clone, Copy)]
struct Hooks(Option<notecevia_os::Native>);

impl notecevia_ui::window::WindowHooks for Hooks {
    fn show_system_menu(&self, x: f32, y: f32) -> bool {
        self.0
            .is_some_and(|native| native.show_system_menu(x as i32, y as i32))
    }

    fn toggle_maximize(&self) -> Option<bool> {
        self.0.and_then(|native| native.toggle_maximize())
    }

    fn is_maximized(&self) -> Option<bool> {
        self.0.and_then(|native| native.is_maximized())
    }
}

struct App {
    workspace: Option<Workspace>,
    home: HomeApp,
    board: Option<BoardApp>,
    /// 外壳：顶栏、两条侧栏与资源栏的宽度。
    shell: Shell,
    /// 回收站视图：删掉的东西与它们的剩余天数。
    trash: TrashView,
    /// 资源栏：工作区的媒体与筛选状态。
    media: MediaPanel,
    /// 界面布局文件的位置。拿到路径才写盘。
    layout_path: Option<PathBuf>,
    /// 上一次设过的窗口标题，变了才发命令。
    window_title: Option<String>,
    /// 平台窗口层。拿不到就是没有：界面会退回 egui 的通用命令。
    native: Option<notecevia_os::Native>,
    /// 已经试过接平台层。只试一次，免得每帧都试。
    native_ready: bool,
    /// 册视图的状态：册列表与目录。
    library: Library,
    /// 当前打开的册。默认是 Empty book。
    volume_id: u64,
    /// 当前视图。外壳会把它改回去，这里跟着变。
    view: ViewMode,
    /// 当前画板的编号，文件名就是它。
    board_id: u64,
    /// 旧文件打不开时关掉写盘，免得用空画板盖掉用户原来那份。
    can_save: bool,
    /// 上一次自动保存的时间，来自 egui 的时钟。
    last_autosave: f64,
    /// 给了路径就截一张图再退出，用来在没有显示器的地方检查界面。
    shot: Option<PathBuf>,
    frames: u32,
    /// 截图回调把结果放这里，下一帧取走。外层是「完成了一次」，内层是可选的提示。
    capture: Arc<Mutex<Option<Option<String>>>>,
    /// 有截图在飞，避免同一帧重复请求。
    capture_busy: bool,
    /// 这次保存之后该重画缩略图。
    want_thumbnail: bool,
    /// 上一次重画缩略图的时间。
    last_thumbnail_at: f64,
    /// 缩略图那一帧用临时视图，下一帧要还原。
    restore_thumbnail_view: bool,
    /// 开发检查用：跑起来先存一次盘，确认缩略图真的会生成。
    thumbnail_check: bool,
    /// 开发检查用：跑起来截一张当前画布，确认导出图片的链路通。
    image_check: bool,
    /// 开发检查用：跑起来把当前画板导出容器与 HTML，确认导出链路通。
    export_check: bool,
    /// boards/ 目录上一次看到的修改时间，用来发现外部改动。
    boards_stamp: Option<SystemTime>,
    /// 上一次检查外部改动的时间。
    last_watch_at: f64,
    /// 开发检查用：打印从进程启动到首帧的耗时。
    timing: bool,
    /// 已经打过耗时了。
    timed: bool,
    /// 进程启动的时刻。
    started: Instant,
    /// 开发检查用：连续采集成帧时间多少秒后打印统计再退出。
    frame_stats: Option<f64>,
    /// 开始采集的时刻。
    stats_started: Option<Instant>,
    /// 上一帧的时刻。
    last_frame_at: Option<Instant>,
    /// 采到的每帧耗时，秒。
    frame_samples: Vec<f64>,
}

impl App {
    fn new(shot: Option<PathBuf>, layout: Layout) -> Self {
        Self {
            workspace: None,
            home: HomeApp::new(),
            board: None,
            shell: Shell::with_layout(layout),
            trash: TrashView::new(),
            media: MediaPanel::new(),
            layout_path: None,
            window_title: None,
            native: None,
            native_ready: false,
            library: Library::new(EMPTY_BOOK_ID),
            volume_id: EMPTY_BOOK_ID,
            view: ViewMode::Page,
            board_id: 1,
            can_save: false,
            last_autosave: 0.0,
            shot,
            frames: 0,
            capture: Arc::new(Mutex::new(None)),
            capture_busy: false,
            want_thumbnail: false,
            last_thumbnail_at: f64::NEG_INFINITY,
            restore_thumbnail_view: false,
            thumbnail_check: false,
            image_check: false,
            export_check: false,
            boards_stamp: None,
            last_watch_at: 0.0,
            timing: false,
            timed: false,
            started: Instant::now(),
            frame_stats: None,
            stats_started: None,
            last_frame_at: None,
            frame_samples: Vec::new(),
        }
    }

    /// 状态提示跟着当前视图走：画布上写画布，其余写首页那条。
    fn status(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.view == ViewMode::Page
            && let Some(board) = &mut self.board
        {
            board.set_status(text);
        } else {
            self.home.set_status(text);
        }
    }

    /// 重读回收站。剩余天数在这里算，界面不读时钟。
    fn refresh_trash(&mut self) {
        self.trash.set_keep_days(TRASH_KEEP_DAYS);
        let Some(workspace) = &self.workspace else {
            self.trash.set_rows(Vec::new());
            return;
        };
        let now = now_seconds();
        match workspace.list_trash() {
            Ok(items) => {
                let rows = items
                    .into_iter()
                    .map(|item| TrashRow {
                        folder: item.folder,
                        kind: match item.kind {
                            SavedTrashKind::Board => TrashKind::Board,
                            SavedTrashKind::Volume => TrashKind::Volume,
                            SavedTrashKind::Shelf => TrashKind::Shelf,
                        },
                        title: item.title,
                        days_left: days_left(item.deleted_at, now),
                        bytes: item.bytes,
                    })
                    .collect();
                self.trash.set_rows(rows);
            }
            Err(error) => {
                self.trash.set_rows(Vec::new());
                let message = strings::trash_list_failed(error);
                self.status(message);
            }
        }
    }

    fn handle_trash_action(&mut self, action: TrashAction) {
        match action {
            TrashAction::Restore(folder) => {
                let Some(workspace) = &self.workspace else {
                    return;
                };
                match workspace.restore_trash(&folder) {
                    Ok(()) => {
                        self.status(strings::TRASH_RESTORED);
                        // 还原的是笔记就回到列表，是册就回到册列表：两处都刷一遍。
                        self.refresh_boards();
                        self.refresh_library();
                    }
                    Err(error) => {
                        let message = strings::restore_failed(error);
                        self.status(message);
                    }
                }
            }
            TrashAction::Purge(folder) => {
                let Some(workspace) = &self.workspace else {
                    return;
                };
                match workspace.purge_trash_item(&folder) {
                    Ok(()) => self.status(strings::TRASH_PURGED),
                    Err(error) => {
                        let message = strings::purge_failed(error);
                        self.status(message);
                    }
                }
            }
            TrashAction::Empty => {
                let Some(workspace) = &self.workspace else {
                    return;
                };
                match workspace.empty_trash() {
                    Ok(_) => self.status(strings::TRASH_EMPTIED),
                    Err(error) => {
                        let message = strings::empty_trash_failed(error);
                        self.status(message);
                    }
                }
            }
        }
        // 不管成没成，列表都按盘上真实情况重画：失败时也能看见还剩什么。
        self.refresh_trash();
    }

    /// 面板宽度摆完了就写回 ui.json。
    ///
    /// 写不进去只影响下次打开的宽度，不当错误报给用户：这不是内容。
    fn save_layout(&mut self, layout: Layout) {
        let Some(path) = &self.layout_path else {
            return;
        };
        if let Err(error) = std::fs::write(path, layout.to_json()) {
            eprintln!("界面布局写不进去：{error}");
        }
    }

    fn save(&mut self) {
        if !self.can_save {
            self.status(strings::CANNOT_SAVE);
            return;
        }
        let result = match (&self.workspace, &self.board) {
            (Some(workspace), Some(board)) => {
                workspace.save_changed(self.board_id, board.document(), &board.changed_bodies())
            }
            _ => return,
        };
        match result {
            Ok(()) => {
                if let Some(board) = &mut self.board {
                    board.mark_saved();
                    board.set_status(strings::SAVED);
                }
                self.want_thumbnail = true;
                self.note_workspace_stamp();
                // 新插进去的媒体要立刻在资源栏里看得到。
                self.refresh_media();
            }
            Err(error) => self.status(strings::save_failed(error)),
        }
    }

    fn refresh_boards(&mut self) {
        let Some(workspace) = &self.workspace else {
            self.home.set_boards(Vec::new());
            return;
        };
        match workspace.list_boards() {
            Ok(infos) => self
                .home
                .set_boards(infos.into_iter().map(board_item).collect()),
            Err(error) => {
                let message = strings::open_failed(error);
                self.home.set_error(message);
            }
        }
    }

    fn open_board(&mut self, id: u64) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let loaded = workspace.load(id);
        let media = DiskMedia {
            dir: workspace.root().join("media"),
        };
        match loaded {
            Ok(document) => {
                self.board = Some(BoardApp::new(document, Box::new(media)));
                self.board_id = id;
                self.last_autosave = 0.0;
                self.last_thumbnail_at = f64::NEG_INFINITY;
            }
            Err(error) => {
                let message = strings::open_failed(error);
                self.home.set_error(message);
            }
        }
    }

    /// 新建一张空笔记并立刻落盘，这样侧栏立刻能看到它。
    ///
    /// 已经开着一张没写过东西的空笔记时直接复用它：否则点几下「＋」就堆出一排空笔记。
    fn create_board(&mut self) {
        if let Some(board) = &self.board
            && is_blank_note(board)
        {
            self.status(strings::BOARD_ALREADY_EMPTY);
            return;
        }
        self.save();
        let next = match &self.workspace {
            Some(workspace) => workspace.next_board(),
            None => {
                self.status(strings::CANNOT_SAVE);
                return;
            }
        };
        let Ok(id) = next else {
            let message = strings::open_failed("取不到新画板编号");
            self.status(message);
            return;
        };
        let media = DiskMedia {
            dir: match &self.workspace {
                Some(workspace) => workspace.root().join("media"),
                None => PathBuf::from("media"),
            },
        };
        self.board = Some(BoardApp::new(
            Document::new(strings::UNTITLED_NOTE),
            Box::new(media),
        ));
        self.board_id = id;
        self.last_autosave = 0.0;
        self.last_thumbnail_at = f64::NEG_INFINITY;
        self.save();
        // 侧栏一直挂在那儿，新建完得让列表立刻看见，否则会显示"还没有笔记"。
        self.refresh_library();
        self.note_workspace_stamp();
        self.status(strings::BOARD_CREATED);
    }

    fn open_latest_or_create(&mut self) {
        let latest = self
            .workspace
            .as_ref()
            .and_then(|workspace| workspace.load_latest().ok().flatten())
            .map(|(id, _)| id);
        match latest {
            Some(id) => self.open_board(id),
            None => self.create_board(),
        }
    }

    fn go_home(&mut self) {
        self.save();
        self.board = None;
        self.refresh_boards();
    }

    fn handle_home_action(&mut self, action: HomeAction) {
        match action {
            HomeAction::Create => self.create_board(),
            HomeAction::Open(id) => self.open_board(id),
            HomeAction::Delete(id) => {
                let result = match &self.workspace {
                    Some(workspace) => workspace.delete_board(id),
                    None => return,
                };
                match result {
                    Ok(()) => self.home.set_status(strings::BOARD_DELETED),
                    Err(error) => {
                        let message = strings::open_failed(error);
                        self.home.set_error(message);
                    }
                }
                self.refresh_boards();
                self.note_workspace_stamp();
            }
            HomeAction::Rename(id, title) => {
                self.update_board(
                    id,
                    |document| document.board.title = title,
                    strings::BOARD_RENAMED,
                );
            }
            HomeAction::SetTags(id, tags) => {
                self.update_board(
                    id,
                    |document| document.board.tags = tags,
                    strings::TAGS_SAVED,
                );
            }
            HomeAction::Export(id, kind) => self.export(id, kind),
            HomeAction::Search(query) => self.search(&query),
            HomeAction::Refresh => self.refresh_index(),
        }
    }

    /// 外部程序改过工作区时，强制重建索引并刷新列表。
    fn refresh_index(&mut self) {
        let result = match &self.workspace {
            Some(workspace) => workspace.rebuild_index(),
            None => return,
        };
        match result {
            Ok(_) => self.home.set_status(strings::REFRESHED),
            Err(error) => {
                let message = strings::open_failed(error);
                self.home.set_error(message);
            }
        }
        self.refresh_boards();
        self.note_workspace_stamp();
    }

    /// boards/ 目录的修改时间。新建、删除、改名与原子写都会动它。
    fn workspace_stamp(&self) -> Option<SystemTime> {
        let dir = self.workspace.as_ref()?.root().join("boards");
        std::fs::metadata(dir).ok()?.modified().ok()
    }

    /// 自己的写盘也会动目录时间，记一下，免得回家时把自己当成外部改动。
    fn note_workspace_stamp(&mut self) {
        self.boards_stamp = self.workspace_stamp();
    }

    /// 停在首页时每两秒看一次 boards/，外部改过就重建索引并刷新列表。
    fn watch_external_changes(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|input| input.time);
        if now - self.last_watch_at >= WATCH_SECONDS {
            self.last_watch_at = now;
            let stamp = self.workspace_stamp();
            if let (Some(previous), Some(current)) = (self.boards_stamp, stamp)
                && previous != current
            {
                self.refresh_index();
            }
            self.boards_stamp = stamp;
        }
        ctx.request_repaint_after(Duration::from_secs(2));
    }

    /// 读出画板、改一处、写回去。重命名和改标签都走这条路。
    fn update_board(&mut self, id: u64, change: impl FnOnce(&mut Document), done: &'static str) {
        let result = match &self.workspace {
            Some(workspace) => workspace.load(id),
            None => return,
        };
        let mut document = match result {
            Ok(document) => document,
            Err(error) => {
                let message = strings::open_failed(error);
                self.home.set_error(message);
                return;
            }
        };
        change(&mut document);
        let result = match &self.workspace {
            Some(workspace) => workspace.save(id, &document),
            None => return,
        };
        match result {
            Ok(()) => self.home.set_status(done),
            Err(error) => {
                let message = strings::save_failed(error);
                self.home.set_error(message);
            }
        }
        self.refresh_boards();
        self.note_workspace_stamp();
    }

    /// 当前册的名字。
    fn volume_title(&self) -> String {
        self.library
            .volumes()
            .iter()
            .find(|volume| volume.id == self.volume_id)
            .map(|volume| volume.title.clone())
            .unwrap_or_else(|| strings::SHELL_VIEW_VOLUME.to_owned())
    }

    /// 重新读册列表与当前册的目录。
    fn refresh_library(&mut self) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let volumes = match workspace.list_volumes() {
            Ok(volumes) => volumes,
            Err(error) => {
                let message = strings::open_failed(error);
                self.home.set_error(message);
                return;
            }
        };
        let rows: Vec<VolumeRow> = volumes
            .iter()
            .map(|volume| VolumeRow {
                id: volume.id,
                title: volume.title.clone(),
                pages: volume.page_count,
            })
            .collect();
        self.library.set_volumes(rows);
        self.library.set_current(Some(self.volume_id));

        let pages = workspace
            .pages_in_volume(self.volume_id)
            .unwrap_or_default();
        let infos = workspace.list_boards().unwrap_or_default();
        let page_rows: Vec<PageRow> = pages
            .iter()
            .filter_map(|id| {
                infos
                    .iter()
                    .find(|info| info.id == *id)
                    .map(|info| PageRow {
                        id: *id,
                        title: info.title.clone(),
                    })
            })
            .collect();
        self.library.set_pages(page_rows);

        // 页视图列的也是"当前册的页"，两处不能各说各的。
        let order: Vec<u64> = pages;
        let mut items: Vec<BoardItem> = Vec::new();
        for id in order {
            if let Some(info) = infos.iter().find(|info| info.id == id) {
                items.push(board_item(info.clone()));
            }
        }
        self.home.set_boards(items);
    }

    /// 册视图按出来的动作。
    fn handle_library_action(&mut self, action: LibraryAction) {
        match action {
            LibraryAction::New => self.new_volume(),
            LibraryAction::Open(id) => {
                self.volume_id = id;
                self.refresh_library();
            }
            LibraryAction::Delete(id) => {
                let result = match &self.workspace {
                    Some(workspace) => workspace.delete_volume(id),
                    None => return,
                };
                match result {
                    Ok(()) => {
                        self.home.set_status(strings::BOARD_DELETED);
                        if self.volume_id == id {
                            self.volume_id = EMPTY_BOOK_ID;
                        }
                        self.refresh_boards();
                        self.refresh_library();
                    }
                    Err(error) => self.home.set_error(strings::open_failed(error)),
                }
            }
            LibraryAction::Rename(id, title) => {
                let changed = match &self.workspace {
                    Some(workspace) => workspace.load_volume(id).map(|mut volume| {
                        volume.title = title;
                        volume
                    }),
                    None => return,
                };
                if let Ok(volume) = changed {
                    if let Some(workspace) = &self.workspace {
                        let result = workspace.save_volume(id, &volume);
                        if let Err(error) = result {
                            self.home.set_error(strings::save_failed(error));
                            return;
                        }
                    }
                    self.refresh_library();
                }
            }
            LibraryAction::OpenPage(page) => {
                self.open_board(page);
                self.view = ViewMode::Page;
                self.shell.set_view(ViewMode::Page);
            }
            LibraryAction::RemoveFromVolume(page) => {
                let result = match &self.workspace {
                    Some(workspace) => workspace.unassign_page(page),
                    None => return,
                };
                if let Err(error) = result {
                    self.home.set_error(strings::save_failed(error));
                }
                self.refresh_library();
            }
        }
    }

    /// 新建一本册并打开它。
    fn new_volume(&mut self) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let next = workspace.next_volume();
        let Ok(id) = next else {
            self.home.set_error(strings::open_failed("取不到新册编号"));
            return;
        };
        let volume = notecevia_core::Volume::new(strings::LIBRARY_UNTITLED);
        if let Err(error) = workspace.save_volume(id, &volume) {
            self.home.set_error(strings::save_failed(error));
            return;
        }
        self.volume_id = id;
        self.refresh_library();
    }

    fn search(&mut self, query: &str) {
        if query.trim().is_empty() {
            self.refresh_boards();
            return;
        }
        let Some(workspace) = &self.workspace else {
            return;
        };
        let infos = workspace.list_boards().unwrap_or_default();
        let hits = match workspace.search(query) {
            Ok(hits) => hits,
            Err(error) => {
                let message = strings::open_failed(error);
                self.home.set_error(message);
                return;
            }
        };
        let items = hits
            .into_iter()
            .map(|hit| {
                let info = infos.iter().find(|info| info.id == hit.board_id);
                BoardItem {
                    id: hit.board_id,
                    title: hit.title,
                    tags: info.map(|info| info.tags.clone()).unwrap_or_default(),
                    snippet: Some(hit.snippet),
                    updated_at: info.map_or(0, |info| info.updated_at),
                    entry_count: info.map_or(0, |info| info.entry_count),
                }
            })
            .collect();
        self.home.set_boards(items);
    }

    /// 导出到工作区里的 exports/，路径回显在状态栏。
    fn export(&mut self, id: u64, kind: ExportKind) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let dir = workspace.root().join("exports");
        if let Err(error) = std::fs::create_dir_all(&dir) {
            self.home.set_error(strings::export_failed(error));
            return;
        }
        let title = workspace
            .list_boards()
            .unwrap_or_default()
            .into_iter()
            .find(|info| info.id == id)
            .map(|info| info.title)
            .unwrap_or_default();
        let stem = format!("{}-{id}", slug(&title));
        let (path, result) = match kind {
            ExportKind::Container => {
                let path = dir.join(format!("{stem}.notecevia"));
                let result = workspace.export_container(id, &path);
                (path, result)
            }
            ExportKind::Html => {
                let path = dir.join(format!("{stem}.html"));
                let result = workspace.export_html(id, &path);
                (path, result)
            }
        };
        match result {
            Ok(()) => self.home.set_status(strings::exported(path.display())),
            Err(error) => self.home.set_error(strings::export_failed(error)),
        }
    }

    /// 重读资源库。索引里那张媒体表是现成的，这里只换成界面要的形状。
    fn refresh_media(&mut self) {
        let Some(workspace) = &self.workspace else {
            self.media.set_rows(Vec::new());
            return;
        };
        match workspace.list_media() {
            Ok(items) => {
                let rows = items
                    .into_iter()
                    .filter_map(|item| {
                        let kind = MediaKind::of(item.kind)?;
                        Some(MediaRow {
                            hash: item.hash,
                            kind,
                            name: item.name,
                            bytes: item.bytes,
                            boards: item.boards,
                        })
                    })
                    .collect();
                self.media.set_rows(rows);
            }
            Err(error) => {
                self.media.set_rows(Vec::new());
                let message = strings::media_list_failed(error);
                self.status(message);
            }
        }
    }

    /// 资源栏里按出来的动作：放进笔记，或者开系统文件选择器导进来。
    fn handle_media_action(&mut self, action: MediaAction) {
        match action {
            MediaAction::Insert(row) => {
                let kind = match row.kind {
                    MediaKind::Image => BlockKind::Image,
                    MediaKind::Video => BlockKind::Video,
                    MediaKind::Audio => BlockKind::Audio,
                };
                let Some(board) = self.board.as_mut() else {
                    self.status(strings::MEDIA_NEEDS_NOTE);
                    return;
                };
                board.insert_media(kind, row.hash, row.name, row.bytes);
            }
            MediaAction::Import => self.import_media(),
        }
    }

    /// 挑文件导进来：放进媒体池，再各插一块到当前笔记。
    fn import_media(&mut self) {
        if self.board.is_none() {
            self.status(strings::MEDIA_NEEDS_NOTE);
            return;
        }
        let Some(paths) = rfd::FileDialog::new()
            .add_filter(
                strings::MEDIA_IMAGE,
                &["png", "jpg", "jpeg", "gif", "webp", "bmp"],
            )
            .add_filter(strings::MEDIA_VIDEO, &["mp4", "mov", "mkv", "webm", "avi"])
            .add_filter(strings::MEDIA_AUDIO, &["mp3", "wav", "flac", "ogg", "m4a"])
            .pick_files()
        else {
            return;
        };
        let mut imported = 0;
        for path in paths {
            if self.insert_media_file(&path) {
                imported += 1;
            }
        }
        if imported > 0 {
            let message = strings::media_imported(imported);
            self.status(message);
        }
    }

    /// 把拖进画布的文件复制进媒体池，再插成一块。成功给 true。
    fn insert_media_file(&mut self, path: &Path) -> bool {
        let Some(kind) = media_kind(path) else {
            self.status(strings::UNSUPPORTED_FILE);
            return false;
        };
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                let message = format!("{}：{error}", strings::MEDIA_READ_FAILED);
                self.status(message);
                return false;
            }
        };
        let Some(workspace) = &self.workspace else {
            return false;
        };
        let hash = match workspace.put_media(&name, &bytes) {
            Ok(hash) => hash,
            Err(error) => {
                let message = strings::save_failed(error);
                self.status(message);
                return false;
            }
        };
        let size = bytes.len() as u64;
        let Some(board) = &mut self.board else {
            return false;
        };
        board.insert_media(kind, hash, name, size);
        true
    }

    fn import_container(&mut self, path: &Path) {
        let Some(workspace) = &self.workspace else {
            return;
        };
        let result = workspace.import_container(path);
        match result {
            Ok(_) => self.home.set_status(strings::IMPORTED),
            Err(error) => {
                let message = strings::open_failed(error);
                self.home.set_error(message);
            }
        }
        self.refresh_boards();
        self.note_workspace_stamp();
    }

    /// 双击媒体块时用系统默认程序打开，不自己解码播放。
    fn open_media_request(&mut self) {
        let Some(request) = self.board.as_mut().and_then(BoardApp::take_media_request) else {
            return;
        };
        let Some(workspace) = &self.workspace else {
            return;
        };
        let path = workspace.media_path(&request);
        if let Err(error) = open_with_system(&path) {
            let message = format!("{}：{error}", strings::MEDIA_MISSING);
            self.status(message);
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .filter_map(|file| {
                    let path = file.path();
                    (!path.as_os_str().is_empty()).then(|| path.to_path_buf())
                })
                .collect()
        });
        for path in dropped {
            if self.board.is_some() {
                self.insert_media_file(&path);
            } else if is_container(&path) {
                self.import_container(&path);
            }
        }
    }

    fn autosave(&mut self, ctx: &egui::Context) {
        let dirty = self.board.as_ref().is_some_and(BoardApp::is_dirty);
        if !dirty {
            return;
        }
        let now = ctx.input(|input| input.time);
        if now - self.last_autosave < AUTOSAVE_SECONDS {
            ctx.request_repaint_after(Duration::from_millis(400));
            return;
        }
        self.last_autosave = now;
        self.save();
    }

    /// 连续采一段时间帧时间，打印 min/p50/p95/max 后关窗。给「输入到可见」量下界。
    fn collect_frame_stats(&mut self, ctx: &egui::Context) {
        let Some(seconds) = self.frame_stats else {
            return;
        };
        let now = Instant::now();
        let started = *self.stats_started.get_or_insert(now);
        if let Some(last) = self.last_frame_at {
            let delta = now.duration_since(last).as_secs_f64();
            if delta >= 0.1 {
                eprintln!(
                    "慢帧 #{}：{:.1} ms",
                    self.frame_samples.len() + 1,
                    delta * 1000.0
                );
            }
            self.frame_samples.push(delta);
        }
        self.last_frame_at = Some(now);
        if now.duration_since(started).as_secs_f64() < seconds {
            ctx.request_repaint();
            return;
        }
        let mut samples = std::mem::take(&mut self.frame_samples);
        samples.sort_by(f64::total_cmp);
        let percentile = |fraction: f64| -> f64 {
            if samples.is_empty() {
                return 0.0;
            }
            let index = ((samples.len() - 1) as f64 * fraction).round() as usize;
            samples[index] * 1000.0
        };
        let mean = if samples.is_empty() {
            0.0
        } else {
            samples.iter().sum::<f64>() / samples.len() as f64 * 1000.0
        };
        eprintln!(
            "帧时间 {} 帧：min {:.1} ms，mean {:.1} ms，p50 {:.1} ms，p95 {:.1} ms，max {:.1} ms",
            samples.len(),
            percentile(0.0),
            mean,
            percentile(0.5),
            percentile(0.95),
            percentile(1.0),
        );
        self.frame_stats = None;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// 把截图回调的结果取回来，顺手清掉在飞标记。
    fn take_capture_result(&mut self) {
        let outcome = self.capture.lock().ok().and_then(|mut slot| slot.take());
        let Some(message) = outcome else {
            return;
        };
        self.capture_busy = false;
        if let Some(message) = message {
            self.status(message);
        }
    }

    /// 画布顶栏点了「导出图片」：截当前画布，存到工作区的 exports/。
    fn handle_image_export(&mut self, ctx: &egui::Context) {
        if self.shot.is_some() || self.capture_busy {
            return;
        }
        let Some(board) = self.board.as_mut() else {
            return;
        };
        if !board.take_image_export_request() {
            return;
        }
        let rect = board.canvas_rect();
        let title = board.document().board.title.clone();
        let Some(dir) = self.workspace.as_ref().map(|it| it.root().join("exports")) else {
            return;
        };
        if let Err(error) = std::fs::create_dir_all(&dir) {
            self.status(strings::export_failed(error));
            return;
        }
        let path = dir.join(format!("{}-{}.png", slug(&title), self.board_id));
        self.status(strings::EXPORTING);
        self.request_capture(ctx, path, rect, None, true);
    }

    /// 该不该为缩略图把视图缩到全貌。窗口聚焦时不做，免得用户看到一帧缩放闪动。
    fn should_fit_thumbnail(&self, ctx: &egui::Context) -> bool {
        if !self.want_thumbnail || self.capture_busy || self.restore_thumbnail_view {
            return false;
        }
        if self.board.as_ref().is_some_and(BoardApp::is_dirty) {
            return false;
        }
        if self.shot.is_some() && !self.thumbnail_check {
            return false;
        }
        if ctx.input(|input| input.focused) && !self.thumbnail_check {
            return false;
        }
        let now = ctx.input(|input| input.time);
        now - self.last_thumbnail_at >= THUMBNAIL_SECONDS
    }

    /// 截一张当前画布的缩略图存进工作区。调用前视图已经被缩到内容全貌。
    fn capture_thumbnail(&mut self, ctx: &egui::Context) {
        self.want_thumbnail = false;
        self.last_thumbnail_at = ctx.input(|input| input.time);
        let rect = self.board.as_ref().map(BoardApp::canvas_rect);
        let dir = self
            .workspace
            .as_ref()
            .map(|it| it.root().join("thumbnails"));
        let (Some(rect), Some(dir)) = (rect, dir) else {
            self.restore_thumbnail_view = true;
            return;
        };
        if std::fs::create_dir_all(&dir).is_err() {
            self.restore_thumbnail_view = true;
            return;
        }
        let path = dir.join(format!("{}.png", self.board_id));
        self.request_capture(ctx, path, rect, Some(THUMBNAIL_WIDTH), false);
        self.restore_thumbnail_view = true;
    }

    /// 请求一帧截图，按给定画布矩形裁剪后写成 PNG。
    fn request_capture(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        rect: egui::Rect,
        max_width: Option<u32>,
        announce: bool,
    ) {
        let pixels_per_point = ctx.pixels_per_point();
        let slot = Arc::clone(&self.capture);
        let display = path.display().to_string();
        self.capture_busy = true;
        ctx.request_screenshot(move |image| {
            let outcome = save_cropped_png(&path, &image, rect, pixels_per_point, max_width);
            let message = match outcome {
                Ok(()) if announce => Some(strings::image_exported(display)),
                Ok(()) => None,
                Err(error) => Some(strings::export_failed(error)),
            };
            if let Ok(mut slot) = slot.lock() {
                *slot = Some(message);
            }
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.timing && !self.timed {
            self.timed = true;
            eprintln!("冷启动到首帧：{} ms", self.started.elapsed().as_millis());
        }
        self.collect_frame_stats(&ctx);
        self.take_capture_result();
        // 平台层接一次。接不上就把系统标题栏放回来：宁可多一条栏，
        // 也不能留一个拖不动、拉不了边的窗口给用户。
        if !self.native_ready {
            self.native_ready = true;
            match frame.window_handle() {
                Ok(handle) => match notecevia_os::Native::attach(handle.as_raw()) {
                    Some(native) => {
                        native.allow_resize();
                        self.native = Some(native);
                    }
                    None => ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(true)),
                },
                Err(_) => ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(true)),
            }
        }
        let new_note_key =
            ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::N));
        // 界不读时钟：这里把"现在"递进去，列表才好写"3 分钟前"。
        let now = now_seconds();
        self.home.set_now(now);
        // 当前东西的名字放窗口标题栏：界面里再顶一个名字，就成标签页了。
        let title = match &self.board {
            Some(board) if self.view == ViewMode::Page => {
                format!("{} — Notecevia", board.document().board.title)
            }
            _ if self.view == ViewMode::Volume => format!("{} — Notecevia", self.volume_title()),
            _ if self.view == ViewMode::Trash => format!("{} — Notecevia", strings::TRASH_TITLE),
            _ => "Notecevia".to_owned(),
        };
        // 系统标题栏没了，同一个名字要写在两处：系统那份给任务栏与 Alt+Tab，
        // 界面这份给窗口里看得见的那行。少了后者，用户认不出自己开着哪一页。
        self.shell.set_title(title.clone());
        if self.window_title.as_deref() != Some(title.as_str()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = Some(title);
        }
        self.home
            .set_current(self.board.as_ref().map(|_| self.board_id));
        let mut fits_thumbnail = false;
        if self.board.is_some() {
            if std::mem::take(&mut self.restore_thumbnail_view)
                && let Some(board) = self.board.as_mut()
            {
                board.end_thumbnail_view();
            }
            fits_thumbnail = self.should_fit_thumbnail(&ctx);
            if fits_thumbnail && let Some(board) = self.board.as_mut() {
                board.begin_thumbnail_view();
            }
        }

        let hooks = Hooks(self.native);
        let views = notecevia_ui::shell::Views {
            home: &mut self.home,
            library: Some(&mut self.library),
            trash: &mut self.trash,
            media_panel: &mut self.media,
            board: self.board.as_mut(),
        };
        let output = self.shell.ui(ui, &hooks, views);
        if let Some(layout) = output.layout {
            self.save_layout(layout);
        }
        if output.new_note {
            self.create_board();
        }
        // Ctrl N 只在这里处理一次。以前侧栏和画布各处理一遍，一次按键建两张。
        if new_note_key && !output.new_note {
            self.create_board();
        }
        match output.menu {
            None => {}
            Some(MenuCommand::NewNote) => self.create_board(),
            Some(MenuCommand::CloseNote) => self.go_home(),
            Some(MenuCommand::ExportImage) => {
                if let Some(board) = self.board.as_mut() {
                    board.request_image_export();
                }
            }
            Some(MenuCommand::ExportHtml) => {
                if self.board.is_some() {
                    self.export(self.board_id, ExportKind::Html);
                }
            }
            Some(MenuCommand::ExportContainer) => {
                if self.board.is_some() {
                    self.export(self.board_id, ExportKind::Container);
                }
            }
            Some(MenuCommand::Undo) => {
                if let Some(board) = self.board.as_mut() {
                    board.undo_step();
                }
            }
            Some(MenuCommand::Redo) => {
                if let Some(board) = self.board.as_mut() {
                    board.redo_step();
                }
            }
            Some(MenuCommand::DeleteSelection) => {
                if let Some(board) = self.board.as_mut() {
                    board.delete_selection();
                }
            }
            Some(MenuCommand::SelectAll) => {
                if let Some(board) = self.board.as_mut() {
                    board.select_everything();
                }
            }
            Some(MenuCommand::ClearSelection) => {
                if let Some(board) = self.board.as_mut() {
                    board.clear_selection();
                }
            }
            Some(MenuCommand::Quit) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Some(MenuCommand::About) => self.home.set_status(strings::about()),
            // 这两个外壳已经自己处理完了。
            Some(MenuCommand::ToggleList | MenuCommand::ToggleAssets) => {}
        }
        if let Some(action) = output.home {
            self.handle_home_action(action);
        }
        if let Some(view) = output.view {
            self.view = view;
            match view {
                ViewMode::Volume => self.refresh_library(),
                // 每次进回收站都重读一遍：别处删了东西，这边要立刻对得上。
                ViewMode::Trash => self.refresh_trash(),
                _ => {}
            }
        }
        if let Some(action) = output.library {
            self.handle_library_action(action);
        }
        if let Some(action) = output.trash {
            self.handle_trash_action(action);
        }
        if let Some(action) = output.media {
            self.handle_media_action(action);
        }

        if self.board.is_some() {
            if fits_thumbnail {
                self.capture_thumbnail(&ctx);
            }
            if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::S)) {
                self.save();
            }
            self.autosave(&ctx);
            self.handle_image_export(&ctx);
            if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::H)) {
                self.go_home();
            }
            self.open_media_request();
            self.handle_dropped_files(&ctx);
        } else {
            self.handle_dropped_files(&ctx);
            self.watch_external_changes(&ctx);
            if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::R)) {
                self.refresh_index();
            }
        }

        if ctx.input(|input| input.viewport().close_requested()) {
            self.save();
        }

        let Some(path) = self.shot.clone() else {
            return;
        };
        self.frames += 1;
        ctx.request_repaint();
        if self.frames == 10 && self.thumbnail_check {
            self.save();
        }
        if self.frames == 8 && self.export_check && self.board.is_some() {
            self.export(self.board_id, ExportKind::Container);
            self.export(self.board_id, ExportKind::Html);
        }
        if self.frames == 15 && self.image_check {
            let rect = self.board.as_ref().map(BoardApp::canvas_rect);
            let dir = self.workspace.as_ref().map(|it| it.root().join("exports"));
            if let (Some(rect), Some(dir)) = (rect, dir) {
                let _ = std::fs::create_dir_all(&dir);
                self.request_capture(&ctx, dir.join("export-check.png"), rect, None, true);
            }
        }
        if self.frames == 25 {
            ctx.request_screenshot(move |image| save_png(&path, &image));
        }
        if self.frames > 40 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        let _ = visuals;
        // 透明：窗口四角交给自绘的圆角底，系统那边不要铺一层不透明的颜色。
        egui::Rgba::TRANSPARENT.to_array()
    }
}

/// 应用图标：近黑方块里一个白色小写 n。顶栏左上角那个标记与它同一个形状。
const ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

/// 把图标 PNG 解成 egui 要的 RGBA。
fn app_icon() -> Option<egui::IconData> {
    let image = image::load_from_memory(ICON_PNG).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

/// 一张没起名、也没放过任何块的笔记。
fn is_blank_note(board: &BoardApp) -> bool {
    let document = board.document();
    document.board.title == strings::UNTITLED_NOTE
        && document.board.entries.is_empty()
        && !board.is_dirty()
}

/// 解析 "宽x高" 这样的窗口尺寸，给检查界面用。
fn parse_size(value: &std::ffi::OsStr) -> Option<[f32; 2]> {
    let text = value.to_str()?;
    let (width, height) = text.split_once(['x', 'X'])?;
    Some([width.parse().ok()?, height.parse().ok()?])
}

/// 现在的 Unix 秒。界面不读时钟，时间由这里递进去。
fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

fn board_item(info: notecevia_store::BoardInfo) -> BoardItem {
    BoardItem {
        id: info.id,
        title: info.title,
        tags: info.tags,
        snippet: None,
        updated_at: info.updated_at,
        entry_count: info.entry_count,
    }
}

fn main() -> eframe::Result {
    let mut shot = None;
    let mut stress = false;
    let mut edit = false;
    let mut select = false;
    let mut home = false;
    let mut volume_view = false;
    let mut trash_view = false;
    let mut thumbnail_check = false;
    let mut image_check = false;
    let mut export_check = false;
    let mut timing = false;
    let mut frame_stats = None;
    let mut nudge = false;
    // 检查界面的小窗口与大字：--size 900x520、--zoom 1.5。
    let mut size: Option<[f32; 2]> = None;
    let mut zoom: Option<f32> = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--shot") => shot = args.next().map(PathBuf::from),
            Some("--stress") => stress = true,
            Some("--edit") => edit = true,
            Some("--select") => select = true,
            Some("--home") => home = true,
            Some("--volume") => volume_view = true,
            Some("--trash") => trash_view = true,
            Some("--thumbnail") => thumbnail_check = true,
            Some("--export-image") => image_check = true,
            Some("--export-all") => export_check = true,
            Some("--timing") => timing = true,
            Some("--nudge") => nudge = true,
            Some("--size") => size = args.next().and_then(|value| parse_size(&value)),
            Some("--zoom") => {
                zoom = args
                    .next()
                    .and_then(|value| value.to_str().and_then(|text| text.parse().ok()));
            }
            Some("--frame-stats") => {
                frame_stats = args
                    .next()
                    .and_then(|seconds| seconds.to_str().and_then(|value| value.parse().ok()));
            }
            Some(other) => eprintln!("忽略不认识的参数：{other}"),
            None => {}
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title(notecevia_ui::window_title())
        .with_inner_size(size.unwrap_or([1280.0, 760.0]))
        .with_min_inner_size([720.0, 420.0])
        // 外面那层系统标题栏不要：头部自己画，品牌只出现一次。
        // 拖动、拉边、最大化由 notecevia-os 补回来，接不上会自动退回系统标题栏。
        .with_decorations(false)
        // 圆角靠透明：窗口本身方，自绘的底是一张圆角卡片，四角透出桌面。
        .with_transparent(true);
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let started = Instant::now();
    eframe::run_native(
        "Notecevia",
        options,
        Box::new(move |cc| {
            if let Some(zoom) = zoom {
                cc.egui_ctx.set_zoom_factor(zoom);
            }
            let root = notecevia_store::workspace_root();
            let (theme, theme_warning) = theme::load(&FileTheme {
                path: root.join("theme.json"),
            });
            let _ = theme::install(theme);
            theme::apply(&cc.egui_ctx);
            let layout_path = root.join("ui.json");
            let (layout, layout_warning) = notecevia_ui::layout::load(&FileLayout {
                path: layout_path.clone(),
            });

            let opened_at;
            let listed_at;
            let mut app = App::new(shot.clone(), layout);
            app.layout_path = Some(layout_path);
            app.thumbnail_check = thumbnail_check;
            app.image_check = image_check;
            app.export_check = export_check;
            app.timing = timing;
            app.frame_stats = frame_stats;
            app.started = started;
            match Workspace::open(&root) {
                Ok(workspace) => {
                    opened_at = started.elapsed();
                    let recovered = workspace.recover().unwrap_or(0);
                    app.workspace = Some(workspace);
                    app.home = HomeApp::with_thumbnails(Box::new(DiskThumbnails {
                        dir: root.join("thumbnails"),
                    }));
                    app.can_save = true;
                    app.refresh_boards();
                    app.refresh_library();
                    app.refresh_media();
                    listed_at = started.elapsed();
                    app.note_workspace_stamp();
                    if recovered > 0 {
                        app.home.set_status(strings::RECOVERED);
                    }
                }
                Err(error) => {
                    opened_at = started.elapsed();
                    listed_at = opened_at;
                    let message = strings::workspace_failed(root.display(), error);
                    app.home.set_error(message);
                }
            }

            // 主题文件写错了要说出来，但界面照常起，用内置浅色。
            if let Some(warning) = theme_warning {
                eprintln!("主题文件读不了，改用内置浅色：{warning}");
                app.home.set_error(warning);
            }
            // 布局文件坏了不值得打扰：退回默认宽度，只在终端说一句。
            if let Some(warning) = layout_warning {
                eprintln!("界面布局读不了，改用默认宽度：{warning}");
            }

            if timing {
                eprintln!(
                    "冷启动：打开工作区 {} ms，列表 {} ms",
                    opened_at.as_millis(),
                    listed_at.as_millis()
                );
            }

            // 截图与交互检查要直接进画布；普通启动停在首页。
            if volume_view {
                app.view = ViewMode::Volume;
                app.refresh_library();
                app.shell.set_view(ViewMode::Volume);
            }
            if trash_view {
                app.view = ViewMode::Trash;
                app.refresh_trash();
                app.shell.set_view(ViewMode::Trash);
            }
            let force_board = !home
                && !volume_view
                && !trash_view
                && (stress || edit || select || nudge || app.shot.is_some());
            if force_board {
                app.open_latest_or_create();
            }
            if let Some(board) = &mut app.board {
                if stress {
                    board.spawn_stress();
                }
                if nudge {
                    board.nudge_first_text();
                }
                if select {
                    board.select_first();
                }
                if edit {
                    board.begin_editing_first();
                }
            }
            Ok(Box::new(app))
        }),
    )
}

fn media_kind(path: &Path) -> Option<BlockKind> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" | "jpg" | "jpeg" => Some(BlockKind::Image),
        "mp4" | "mov" | "webm" | "mkv" => Some(BlockKind::Video),
        "mp3" | "wav" | "ogg" | "flac" | "m4a" => Some(BlockKind::Audio),
        _ => None,
    }
}

fn is_container(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("notecevia")
    )
}

fn is_hash(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// 导出文件名用：保留字母数字与汉字，其余压成一个连字符。
fn slug(title: &str) -> String {
    let mut slug = String::new();
    for character in title.chars() {
        if character.is_alphanumeric() || character == '-' || character == '_' {
            slug.push(character);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "board".to_owned()
    } else {
        slug.to_owned()
    }
}

fn open_with_system(path: &Path) -> std::io::Result<()> {
    if !path.exists() {
        return Err(std::io::Error::from(std::io::ErrorKind::NotFound));
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", ""])
            .arg(path)
            .spawn()
            .map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn().map(|_| ())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open").arg(path).spawn().map(|_| ())
    }
}

/// 把整窗截图切成画布那块，按需缩小，写成 PNG。
fn save_cropped_png(
    path: &Path,
    image: &egui::ColorImage,
    rect: egui::Rect,
    pixels_per_point: f32,
    max_width: Option<u32>,
) -> std::io::Result<()> {
    let [width, height] = image.size;
    if width == 0 || height == 0 {
        return Err(std::io::Error::other("截图是空的"));
    }
    let mut rgba = Vec::with_capacity(width * height * 4);
    for pixel in &image.pixels {
        rgba.extend_from_slice(&pixel.to_srgba_unmultiplied());
    }
    let Some(full) = image::RgbaImage::from_raw(width as u32, height as u32, rgba) else {
        return Err(std::io::Error::other("截图像素数量不对"));
    };
    let to_pixel = |value: f32, limit: usize| -> u32 {
        (value * pixels_per_point).round().clamp(0.0, limit as f32) as u32
    };
    let left = to_pixel(rect.min.x, width);
    let top = to_pixel(rect.min.y, height);
    let right = to_pixel(rect.max.x, width);
    let bottom = to_pixel(rect.max.y, height);
    let crop_width = right.saturating_sub(left).max(1);
    let crop_height = bottom.saturating_sub(top).max(1);
    let mut cropped =
        image::imageops::crop_imm(&full, left, top, crop_width, crop_height).to_image();
    if let Some(max_width) = max_width
        && cropped.width() > max_width
    {
        let ratio = max_width as f32 / cropped.width() as f32;
        let target_height = ((cropped.height() as f32) * ratio).round().max(1.0) as u32;
        cropped = image::imageops::resize(
            &cropped,
            max_width,
            target_height,
            image::imageops::FilterType::Triangle,
        );
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    cropped.save(path).map_err(std::io::Error::other)
}

fn save_png(path: &std::path::Path, image: &egui::ColorImage) {
    let [width, height] = image.size;
    let mut rgba = Vec::with_capacity(width * height * 4);
    for pixel in &image.pixels {
        rgba.extend_from_slice(&pixel.to_srgba_unmultiplied());
    }
    let Some(buffer) = image::RgbaImage::from_raw(width as u32, height as u32, rgba) else {
        return;
    };
    let _ = buffer.save(path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_untitled_empty_note_counts_as_blank() {
        let named = BoardApp::new(Document::new(strings::UNTITLED_NOTE), Box::new(()));
        assert!(is_blank_note(&named));
        let titled = BoardApp::new(Document::new("周会记录"), Box::new(()));
        assert!(!is_blank_note(&titled));
    }

    #[test]
    fn slug_keeps_letters_and_collapses_punctuation() {
        assert_eq!(slug("格式样例 0.2.0-alpha.1"), "格式样例-0-2-0-alpha-1");
        assert_eq!(slug("a/b:c?d"), "a-b-c-d");
        assert_eq!(slug("   "), "board");
        assert_eq!(slug(""), "board");
        assert_eq!(slug("已经-合法_name"), "已经-合法_name");
    }

    #[test]
    fn hashes_must_be_64_hex_characters() {
        assert!(is_hash(&"a".repeat(64)));
        assert!(!is_hash(&"a".repeat(63)));
        assert!(!is_hash(&"z".repeat(64)));
        assert!(!is_hash("../../etc/passwd"));
        assert!(!is_hash(""));
    }

    #[test]
    fn media_kind_maps_known_extensions_only() {
        assert_eq!(media_kind(Path::new("a.PNG")), Some(BlockKind::Image));
        assert_eq!(media_kind(Path::new("照片.jpeg")), Some(BlockKind::Image));
        assert_eq!(media_kind(Path::new("a.mp4")), Some(BlockKind::Video));
        assert_eq!(media_kind(Path::new("a.mkv")), Some(BlockKind::Video));
        assert_eq!(media_kind(Path::new("a.m4a")), Some(BlockKind::Audio));
        assert_eq!(media_kind(Path::new("a.txt")), None);
        assert_eq!(media_kind(Path::new("no-extension")), None);
    }

    #[test]
    fn container_extension_is_recognized() {
        assert!(is_container(Path::new("board.notecevia")));
        assert!(!is_container(Path::new("board.html")));
        assert!(!is_container(Path::new("board")));
    }
}
