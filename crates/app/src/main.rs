#![forbid(unsafe_code)]

//! 二进制入口：组装存储与界面。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use eframe::egui;
use notecevia_core::{BlockKind, Document};
use notecevia_store::Workspace;
use notecevia_ui::home::{BoardItem, ExportKind, HomeAction, HomeApp};
use notecevia_ui::media::MediaSource;
use notecevia_ui::{BoardApp, strings};

/// 自动保存的间隔，秒。改动一直在就按这个节奏落盘。
const AUTOSAVE_SECONDS: f64 = 2.0;

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
        }
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
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.board.is_some() {
            if let Some(board) = self.board.as_mut() {
                board.ui(ui);
            }
            if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::S)) {
                self.save();
            }
            if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::N)) {
                self.create_board();
            }
            if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::H)) {
                self.go_home();
            }
            self.open_media_request();
            self.handle_dropped_files(&ctx);
            self.autosave(&ctx);
            let home_requested = self
                .board
                .as_mut()
                .is_some_and(|board| board.take_home_request());
            if home_requested {
                self.go_home();
            }
        } else {
            self.handle_dropped_files(&ctx);
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
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--shot") => shot = args.next().map(PathBuf::from),
            Some("--dark") => dark = true,
            Some("--stress") => stress = true,
            Some("--edit") => edit = true,
            Some("--select") => select = true,
            Some("--home") => home = true,
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
    eframe::run_native(
        "Notecevia",
        options,
        Box::new(move |cc| {
            if dark {
                cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
            }

            let root = notecevia_store::workspace_root();
            let mut app = App::new(shot.clone());
            match Workspace::open(&root) {
                Ok(workspace) => {
                    let recovered = workspace.recover().unwrap_or(0);
                    app.workspace = Some(workspace);
                    app.can_save = true;
                    app.refresh_boards();
                    if recovered > 0 {
                        app.home.set_status(strings::RECOVERED);
                    }
                }
                Err(error) => {
                    let message = strings::workspace_failed(root.display(), error);
                    app.home.set_status(message);
                }
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
