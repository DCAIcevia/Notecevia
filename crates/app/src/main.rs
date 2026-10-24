#![forbid(unsafe_code)]

//! 二进制入口：组装存储与界面。

use std::path::PathBuf;

use eframe::egui;

struct App {
    board: notecevia_ui::BoardApp,
    /// 给了路径就截一张图再退出，用来在没有显示器的地方检查界面。
    shot: Option<PathBuf>,
    frames: u32,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.board.ui(ui);

        let Some(path) = self.shot.clone() else {
            return;
        };
        let ctx = ui.ctx().clone();
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
    let mut image = None;
    let mut shot = None;
    let mut dark = false;
    let mut stress = false;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--shot" {
            shot = args.next().map(PathBuf::from);
        } else if arg == "--dark" {
            dark = true;
        } else if arg == "--stress" {
            stress = true;
        } else {
            image = Some(PathBuf::from(arg));
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
            let mut board = notecevia_ui::BoardApp::new(&cc.egui_ctx, image.as_deref());
            if stress {
                board.spawn_stress();
            }
            Ok(Box::new(App {
                board,
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
