#![forbid(unsafe_code)]

//! 二进制入口：组装存储与界面。

use std::path::PathBuf;

use eframe::egui;
use notecevia_core::Document;
use notecevia_store::Workspace;
use notecevia_ui::BoardApp;

struct App {
    board: BoardApp,
    workspace: Option<Workspace>,
    /// 旧文件打不开时关掉写盘，免得用空画板盖掉用户原来那份。
    can_save: bool,
    /// 给了路径就截一张图再退出，用来在没有显示器的地方检查界面。
    shot: Option<PathBuf>,
    frames: u32,
}

impl App {
    fn save(&mut self) {
        let (Some(workspace), true) = (&self.workspace, self.can_save) else {
            return;
        };
        match workspace.save(self.board.document()) {
            Ok(()) => {
                self.board.mark_saved();
                self.board.set_status("已保存");
            }
            Err(error) => self.board.set_status(format!("保存失败：{error}")),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.board.ui(ui);

        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            self.save();
        }
        if ctx.input(|i| i.viewport().close_requested()) {
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

fn main() -> eframe::Result {
    let mut shot = None;
    let mut dark = false;
    let mut stress = false;
    let mut edit = false;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--shot") => shot = args.next().map(PathBuf::from),
            Some("--dark") => dark = true,
            Some("--stress") => stress = true,
            Some("--edit") => edit = true,
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
            let (workspace, document, can_save, status) = match Workspace::open(&root) {
                Ok(workspace) => match workspace.load() {
                    Ok(document) => (Some(workspace), document, true, None),
                    Err(error) => (
                        Some(workspace),
                        Document::new("画板"),
                        false,
                        Some(format!("打开失败，本次不写盘：{error}")),
                    ),
                },
                Err(error) => (
                    None,
                    Document::new("画板"),
                    false,
                    Some(format!("工作区打不开：{error}")),
                ),
            };

            let mut board = BoardApp::new(document);
            if let Some(status) = status {
                board.set_status(status);
            }
            if stress {
                board.spawn_stress();
            }
            if edit {
                board.begin_editing_first();
            }
            Ok(Box::new(App {
                board,
                workspace,
                can_save,
                shot,
                frames: 0,
            }))
        }),
    )
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
