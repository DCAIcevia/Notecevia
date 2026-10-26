#![forbid(unsafe_code)]

//! 二进制入口：组装存储与界面。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;
use notecevia_core::{BlockKind, Document};
use notecevia_store::Workspace;
use notecevia_ui::home::{BoardItem, ExportKind, HomeAction, HomeApp, ThumbnailSource};
use notecevia_ui::media::MediaSource;
use notecevia_ui::{BoardApp, strings};

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

struct App {
    workspace: Option<Workspace>,
    home: HomeApp,
    board: Option<BoardApp>,
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
    fn new(shot: Option<PathBuf>) -> Self {
        Self {
            workspace: None,
            home: HomeApp::new(),
            board: None,
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

    /// 状态提示跟着当前视图走，画布上写画布，首页写首页。
    fn status(&mut self, text: impl Into<String>) {
        let text = text.into();
        if let Some(board) = &mut self.board {
            board.set_status(text);
        } else {
            self.home.set_status(text);
        }
    }

    fn save(&mut self) {
        if !self.can_save {
            self.status(strings::CANNOT_SAVE);
            return;
        }
        let result = match (&self.workspace, &self.board) {
            (Some(workspace), Some(board)) => workspace.save(self.board_id, board.document()),
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
                self.home.set_status(message);
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
                self.home.set_status(message);
            }
        }
    }

    /// 新建一张空画板并立刻落盘，这样首页列表马上能看到它。
    fn create_board(&mut self) {
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
        self.board = Some(BoardApp::new(Document::new("画板"), Box::new(media)));
        self.board_id = id;
        self.last_autosave = 0.0;
        self.last_thumbnail_at = f64::NEG_INFINITY;
        self.save();
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
                        self.home.set_status(message);
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
                self.home.set_status(message);
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
                self.home.set_status(message);
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
                self.home.set_status(message);
            }
        }
        self.refresh_boards();
        self.note_workspace_stamp();
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
                self.home.set_status(message);
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
            self.home.set_status(strings::export_failed(error));
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
            Err(error) => self.home.set_status(strings::export_failed(error)),
        }
    }

    /// 把拖进画布的文件复制进媒体池，再插成一块。
    fn insert_media_file(&mut self, path: &Path) {
        let Some(kind) = media_kind(path) else {
            self.status(strings::UNSUPPORTED_FILE);
            return;
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
                return;
            }
        };
        let Some(workspace) = &self.workspace else {
            return;
        };
        let hash = match workspace.put_media(&name, &bytes) {
            Ok(hash) => hash,
            Err(error) => {
                let message = strings::save_failed(error);
                self.status(message);
                return;
            }
        };
        let size = bytes.len() as u64;
        if let Some(board) = &mut self.board {
            board.insert_media(kind, hash, name, size);
        }
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
                self.home.set_status(message);
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
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.timing && !self.timed {
            self.timed = true;
            eprintln!("冷启动到首帧：{} ms", self.started.elapsed().as_millis());
        }
        self.collect_frame_stats(&ctx);
        self.take_capture_result();
        if self.board.is_some() {
            if std::mem::take(&mut self.restore_thumbnail_view)
                && let Some(board) = self.board.as_mut()
            {
                board.end_thumbnail_view();
            }
            let fits_thumbnail = self.should_fit_thumbnail(&ctx);
            if fits_thumbnail && let Some(board) = self.board.as_mut() {
                board.begin_thumbnail_view();
            }
            if let Some(board) = self.board.as_mut() {
                board.ui(ui);
            }
            if fits_thumbnail {
                self.capture_thumbnail(&ctx);
            }
            if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::S)) {
                self.save();
            }
            self.autosave(&ctx);
            self.handle_image_export(&ctx);
            if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::N)) {
                self.create_board();
            }
            if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::H)) {
                self.go_home();
            }
            self.open_media_request();
            self.handle_dropped_files(&ctx);
            let home_requested = self
                .board
                .as_mut()
                .is_some_and(|board| board.take_home_request());
            if home_requested {
                self.go_home();
            }
        } else {
            self.handle_dropped_files(&ctx);
            self.watch_external_changes(&ctx);
            if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::R)) {
                self.refresh_index();
            }
            if let Some(action) = self.home.ui(ui) {
                self.handle_home_action(action);
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
        notecevia_ui::theme::palette(visuals.dark_mode)
            .canvas
            .to_normalized_gamma_f32()
    }
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
    let mut dark = false;
    let mut stress = false;
    let mut edit = false;
    let mut select = false;
    let mut home = false;
    let mut thumbnail_check = false;
    let mut image_check = false;
    let mut export_check = false;
    let mut timing = false;
    let mut frame_stats = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--shot") => shot = args.next().map(PathBuf::from),
            Some("--dark") => dark = true,
            Some("--stress") => stress = true,
            Some("--edit") => edit = true,
            Some("--select") => select = true,
            Some("--home") => home = true,
            Some("--thumbnail") => thumbnail_check = true,
            Some("--export-image") => image_check = true,
            Some("--export-all") => export_check = true,
            Some("--timing") => timing = true,
            Some("--frame-stats") => {
                frame_stats = args
                    .next()
                    .and_then(|seconds| seconds.to_str().and_then(|value| value.parse().ok()));
            }
            Some(other) => eprintln!("忽略不认识的参数：{other}"),
            None => {}
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(notecevia_ui::window_title())
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([720.0, 420.0]),
        ..Default::default()
    };
    let started = Instant::now();
    eframe::run_native(
        "Notecevia",
        options,
        Box::new(move |cc| {
            if dark {
                cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
            }

            let root = notecevia_store::workspace_root();
            let opened_at;
            let listed_at;
            let mut app = App::new(shot.clone());
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
                    app.home.set_status(message);
                }
            }

            if timing {
                eprintln!(
                    "冷启动：打开工作区 {} ms，列表 {} ms",
                    opened_at.as_millis(),
                    listed_at.as_millis()
                );
            }

            // 截图与交互检查要直接进画布；普通启动停在首页。
            let force_board = !home && (stress || edit || select || app.shot.is_some());
            if force_board {
                app.open_latest_or_create();
            }
            if let Some(board) = &mut app.board {
                if stress {
                    board.spawn_stress();
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
