//! HTML 导出：一张画板一个自包含文件，图片与音视频用 data URI 内嵌，断网也能看。
//!
//! 只导出看得见的内容：隐藏的块不写。布局按世界坐标绝对定位，旋转用 CSS transform。
//! 媒体缺失时留一句提示，不让整份导出失败。

use std::fmt::Write as _;
use std::path::Path;

use notecevia_core::board::{
    BlockKind, Body, Document, Entry, HighlightBlock, ImageBlock, MediaBlock, ShapeBlock,
    ShapeKind, TableBlock, TextBlock,
};

use crate::base64;
use crate::error::Error;
use crate::format;
use crate::workspace::{Workspace, write_atomic};

const STYLE: &str = "<style>
html,body{margin:0;padding:0;background:#f6f5f2;color:#1f2328;}
.board{position:relative;overflow:hidden;}
.block{position:absolute;box-sizing:border-box;transform-origin:center center;margin:0;}
.text{white-space:pre-wrap;overflow:hidden;line-height:1.35;}
table{border-collapse:collapse;width:100%;height:100%;}
td{border:1px solid rgba(31,35,40,.25);padding:4px 6px;vertical-align:top;font-size:14px;white-space:pre-wrap;}
img,video{width:100%;height:100%;display:block;}
.missing{display:flex;align-items:center;justify-content:center;font:12px sans-serif;color:#8a8f98;border:1px dashed #c9ccd1;}
</style>
";

/// 媒体属于哪一类，决定缺省 MIME 类型。
#[derive(Clone, Copy)]
enum MediaKind {
    Image,
    Video,
    Audio,
}

impl Workspace {
    /// 把一张画板导出成自包含 HTML。路径由调用方定，写到哪都行。
    pub fn export_html(&self, board_id: u64, path: impl AsRef<Path>) -> Result<(), Error> {
        let document = self.load(board_id)?;
        let html = self.render_html(&document);
        write_atomic(path.as_ref(), html.as_bytes())
    }

    fn render_html(&self, document: &Document) -> String {
        let mut html = String::new();
        html.push_str("<!DOCTYPE html>\n<html lang=\"zh\">\n<head>\n<meta charset=\"utf-8\">\n");
        html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
        let _ = writeln!(html, "<title>{}</title>", escape(&document.board.title));
        html.push_str(STYLE);

        let mut visible: Vec<&Entry> = document
            .board
            .entries
            .iter()
            .filter(|entry| entry.visible)
            .collect();
        // 同一个层级按目录顺序，稳定排序会保住它。
        visible.sort_by_key(|entry| entry.z);

        let mut width: f32 = 1.0;
        let mut height: f32 = 1.0;
        for entry in &visible {
            width = width.max(entry.geometry.x + entry.geometry.width);
            height = height.max(entry.geometry.y + entry.geometry.height);
        }

        html.push_str("<body>\n");
        let _ = writeln!(
            html,
            "<div class=\"board\" style=\"width:{width:.0}px;height:{height:.0}px;\">"
        );
        for entry in &visible {
            self.render_block(&mut html, document, entry);
        }
        html.push_str("</div>\n</body>\n</html>\n");
        html
    }

    fn render_block(&self, html: &mut String, document: &Document, entry: &Entry) {
        let geometry = entry.geometry;
        let rotation = if geometry.rotation.abs() > f32::EPSILON {
            format!("transform:rotate({:.4}rad);", geometry.rotation)
        } else {
            String::new()
        };
        let frame = format!(
            "left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;z-index:{};{rotation}",
            geometry.x, geometry.y, geometry.width, geometry.height, entry.z
        );
        let Some(body) = document.body(entry.id) else {
            let _ = writeln!(html, "<div class=\"block\" style=\"{frame}\"></div>");
            return;
        };
        match body {
            Body::Text(text) => self.render_text(html, &frame, text),
            Body::Image(image) => self.render_image(html, &frame, image),
            Body::Media(media) => self.render_media(html, &frame, entry, media),
            Body::Table(table) => render_table(html, &frame, table),
            Body::Shape(shape) => render_shape(html, &frame, shape),
            Body::Highlight(highlight) => render_highlight(html, &frame, highlight),
        }
    }

    fn render_text(&self, html: &mut String, frame: &str, text: &TextBlock) {
        let mut style = String::from(frame);
        style.push_str("white-space:pre-wrap;overflow:hidden;");
        if let Some(font) = &text.font {
            let _ = write!(style, "font-family:'{}';", escape(font));
        }
        if let Some(size) = text.size {
            let _ = write!(style, "font-size:{size:.1}px;");
        }
        if let Some(color) = text.color {
            let _ = write!(style, "color:{};", css_color(color, 1.0));
        }
        if let Some(background) = text.background {
            let _ = write!(style, "background:{};", css_color(background, 1.0));
        }
        let _ = writeln!(
            html,
            "<div class=\"block text\" style=\"{style}\">{}</div>",
            escape(&text.text)
        );
    }

    fn render_image(&self, html: &mut String, frame: &str, image: &ImageBlock) {
        let fit = format::fit_name(image.fit);
        match self.media_bytes(&image.media) {
            Ok(bytes) => {
                let mime = mime_of(&image.name, MediaKind::Image);
                let _ = writeln!(
                    html,
                    "<img class=\"block\" alt=\"{}\" style=\"{frame}object-fit:{fit};\" src=\"data:{mime};base64,{}\">",
                    escape(&image.name),
                    base64::encode(&bytes)
                );
            }
            Err(_) => render_missing(html, frame, &image.name, &image.media),
        }
    }

    fn render_media(&self, html: &mut String, frame: &str, entry: &Entry, media: &MediaBlock) {
        let (kind, tag) = if entry.kind == BlockKind::Video {
            (MediaKind::Video, "video")
        } else {
            (MediaKind::Audio, "audio")
        };
        match self.media_bytes(&media.media) {
            Ok(bytes) => {
                let mime = mime_of(&media.name, kind);
                let _ = writeln!(
                    html,
                    "<{tag} class=\"block\" style=\"{frame}\" controls src=\"data:{mime};base64,{}\"></{tag}>",
                    base64::encode(&bytes)
                );
            }
            Err(_) => render_missing(html, frame, &media.name, &media.media),
        }
    }
}

fn render_table(html: &mut String, frame: &str, table: &TableBlock) {
    let _ = write!(html, "<table class=\"block\" style=\"{frame}\"><tbody>");
    for row in 0..table.rows {
        html.push_str("<tr>");
        for col in 0..table.cols {
            let _ = write!(html, "<td>{}</td>", escape(table.cell(row, col)));
        }
        html.push_str("</tr>");
    }
    html.push_str("</tbody></table>\n");
}

fn render_shape(html: &mut String, frame: &str, shape: &ShapeBlock) {
    let mut style = String::from(frame);
    match shape.shape {
        ShapeKind::Rect | ShapeKind::Ellipse => {
            if let Some(fill) = shape.fill {
                let _ = write!(style, "background:{};", css_color(fill, 1.0));
            }
            if let Some(stroke) = shape.stroke {
                let _ = write!(
                    style,
                    "border:{:.1}px solid {};",
                    shape.stroke_width,
                    css_color(stroke, 1.0)
                );
            }
            if shape.shape == ShapeKind::Ellipse {
                style.push_str("border-radius:50%;");
            }
            let _ = writeln!(html, "<div class=\"block\" style=\"{style}\"></div>");
        }
        ShapeKind::Line => {
            let color = shape.stroke.or(shape.fill).map_or_else(
                || "rgba(31,35,40,1)".to_owned(),
                |color| css_color(color, 1.0),
            );
            let thickness = shape.stroke_width.max(1.0);
            let _ = writeln!(
                html,
                "<div class=\"block\" style=\"{frame}display:flex;align-items:center;\"><div style=\"width:100%;height:{thickness:.1}px;background:{color};\"></div></div>"
            );
        }
    }
}

fn render_highlight(html: &mut String, frame: &str, highlight: &HighlightBlock) {
    let color = css_color(highlight.color, highlight.opacity.clamp(0.0, 1.0));
    let _ = writeln!(
        html,
        "<div class=\"block\" style=\"{frame}background:{color};\"></div>"
    );
}

fn render_missing(html: &mut String, frame: &str, name: &str, media: &str) {
    let label = if name.is_empty() { media } else { name };
    let _ = writeln!(
        html,
        "<div class=\"block missing\" style=\"{frame}\">媒体缺失：{}</div>",
        escape(label)
    );
}

fn mime_of(name: &str, kind: MediaKind) -> &'static str {
    let extension = name
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match (kind, extension.as_str()) {
        (MediaKind::Image, "png") => "image/png",
        (MediaKind::Image, "jpg" | "jpeg") => "image/jpeg",
        (MediaKind::Image, "gif") => "image/gif",
        (MediaKind::Image, "webp") => "image/webp",
        (MediaKind::Image, "bmp") => "image/bmp",
        (MediaKind::Image, "svg") => "image/svg+xml",
        (MediaKind::Video, "webm") => "video/webm",
        (MediaKind::Video, "mov") => "video/quicktime",
        (MediaKind::Audio, "wav") => "audio/wav",
        (MediaKind::Audio, "ogg") => "audio/ogg",
        (MediaKind::Audio, "m4a") => "audio/mp4",
        (MediaKind::Audio, "flac") => "audio/flac",
        (MediaKind::Image, _) => "image/png",
        (MediaKind::Video, _) => "video/mp4",
        (MediaKind::Audio, _) => "audio/mpeg",
    }
}

fn css_color([red, green, blue]: [u8; 3], alpha: f32) -> String {
    format!("rgba({red},{green},{blue},{alpha:.3})")
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use notecevia_core::board::{BlockKind, Body, Document, Geometry, ImageFit};

    use crate::base64;
    use crate::workspace::Workspace;

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    /// 导出测试用独立目录：路径里带进程号和序号，测试之间不会互相踩。
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempRoot {
        path: PathBuf,
    }

    impl TempRoot {
        fn new() -> Self {
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("notecevia-html-{}-{unique}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            Self { path }
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn export_embeds_text_and_media() {
        let root = TempRoot::new();
        let workspace = Workspace::open(&root.path).expect("建立工作区");
        let image = workspace.put_media("photo.png", PNG).expect("放媒体");

        let mut document = Document::new("导出 <测试>");
        let text = document.add_block(BlockKind::Text, Geometry::new(10.0, 10.0, 200.0, 60.0));
        if let Some(Body::Text(block)) = document.body_mut(text) {
            block.text = "正文 & <标签>".to_owned();
            block.color = Some([0x20, 0x30, 0x40]);
        }
        let image_id = document.add_block(BlockKind::Image, Geometry::new(10.0, 80.0, 120.0, 90.0));
        if let Some(Body::Image(block)) = document.body_mut(image_id) {
            block.media = image.clone();
            block.name = "photo.png".to_owned();
            block.fit = ImageFit::Contain;
        }
        let hidden = document.add_block(BlockKind::Text, Geometry::new(0.0, 200.0, 100.0, 40.0));
        if let Some(Body::Text(block)) = document.body_mut(hidden) {
            block.text = "看不见的".to_owned();
        }
        if let Some(entry) = document.entry_mut(hidden) {
            entry.visible = false;
        }
        workspace.save_at(1, &document, 100).expect("存盘");

        let path = root.path.join("board.html");
        workspace.export_html(1, &path).expect("导出");
        let html = fs::read_to_string(&path).expect("读导出");

        assert!(html.contains("<title>导出 &lt;测试&gt;</title>"));
        assert!(html.contains("正文 &amp; &lt;标签&gt;"));
        assert!(html.contains(&format!("data:image/png;base64,{}", base64::encode(PNG))));
        assert!(html.contains("object-fit:contain"));
        assert!(html.contains("rgba(32,48,64,1.000)"));
        // 隐藏的块不导出。
        assert!(!html.contains("看不见的"));
        // 自包含：不引用外部地址。
        assert!(!html.contains("http://") && !html.contains("https://"));
    }
}
