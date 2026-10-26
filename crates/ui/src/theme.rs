#![forbid(unsafe_code)]

//! 界面主题：颜色与尺寸只在这里出现，并且可以从文件里换掉。
//!
//! 三层：
//! - `Palette`：画布里世界坐标的配色（还在代码里，等纸张样式定了再搬）。
//! - `Chrome` + `Ui`：画布之外的界面配色与尺寸，一套浅色作默认值。
//! - `Theme`：名字 + 上面两套，可以从 JSON 覆盖其中任意一个 token。
//!
//! 换主题不动代码：把一个 JSON 放到工作区根目录的 `theme.json`，启动时读它。
//! 文件里只写要改的 token，没写的继承内置默认。写错的 token 一律报错，不猜。

use std::sync::OnceLock;

use egui::{Color32, CornerRadius, Stroke};

/// 一套画布配色。浅色与深色都要成立，界面代码只取这里的值。
pub struct Palette {
    pub canvas: Color32,
    pub block_fill: Color32,
    pub media_fill: Color32,
    pub edge: Color32,
    pub hover_edge: Color32,
    pub text: Color32,
    pub muted: Color32,
    /// 错误状态用的颜色。只表示失败，不表示强调。
    pub danger: Color32,
}

/// 世界坐标里的尺寸。绘制时乘以缩放，画面上才不会随窗口变大变小。
pub const BLOCK_FONT: f32 = 15.0;
pub const SMALL_FONT: f32 = 12.0;
/// 块的最小宽高，保证拉不没。
pub const MIN_BLOCK_WIDTH: f32 = 48.0;
pub const MIN_BLOCK_HEIGHT: f32 = 24.0;
/// 选中后贴着边能拉伸的宽度（屏幕像素）。
pub const GRAB_EDGE: f32 = 5.0;
/// 角外面这段区间用来旋转。
pub const GRAB_BAND: f32 = 18.0;
pub const BLOCK_RADIUS: f32 = 8.0;
pub const EDGE_WIDTH: f32 = 1.0;
pub const HINT_INSET: f32 = 12.0;
pub const HINT_PAD: f32 = 6.0;

/// 选框与选中底色这类临时覆盖层的不透明度。
/// 覆盖层只解释状态，不加边框，也不改变几何。
pub const MARQUEE_ALPHA: u8 = 26;

/// 表格单元格与媒体瓦片的内边距，单位是世界坐标里的点。
pub const CELL_PAD: f32 = 5.0;
pub const TILE_PAD: f32 = 10.0;

/// 界面里能选的文字字号，单位是世界坐标里的点。
pub const TEXT_SIZES: [f32; 6] = [12.0, 14.0, 16.0, 20.0, 26.0, 34.0];

/// 调色板里先摆出来的常见颜色。想用别的就在同一个面板里自己调。
pub const TEXT_COLORS: [[u8; 3]; 7] = [
    [0, 0, 0],
    [110, 110, 118],
    [198, 60, 54],
    [206, 132, 40],
    [58, 140, 88],
    [44, 108, 200],
    [124, 92, 214],
];

pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette {
            canvas: Color32::from_rgb(24, 24, 27),
            block_fill: Color32::from_rgb(33, 33, 37),
            media_fill: Color32::from_rgb(43, 43, 48),
            edge: Color32::from_rgb(52, 52, 58),
            hover_edge: Color32::from_rgb(78, 78, 86),
            text: Color32::from_rgb(233, 233, 236),
            muted: Color32::from_rgb(142, 142, 150),
            danger: Color32::from_rgb(236, 122, 122),
        }
    } else {
        Palette {
            canvas: Color32::from_rgb(242, 242, 244),
            block_fill: Color32::from_rgb(255, 255, 255),
            media_fill: Color32::from_rgb(235, 235, 238),
            edge: Color32::from_rgb(224, 224, 229),
            hover_edge: Color32::from_rgb(196, 196, 203),
            text: Color32::from_rgb(28, 28, 31),
            muted: Color32::from_rgb(110, 110, 118),
            danger: Color32::from_rgb(178, 42, 42),
        }
    }
}

/// 画布之外的界面配色。
///
/// 层次只有三层：`rail` 最外的图标条最深，`panel` 两侧面板居中，`window` 顶栏与画布最白。
/// 容器之间用 1px `line` 划界，界面里不再给容器加边框。
#[derive(Clone, Debug, PartialEq)]
pub struct Chrome {
    pub window: Color32,
    pub panel: Color32,
    pub rail: Color32,
    pub line: Color32,
    pub line_soft: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub text_faint: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub selected: Color32,
    /// 输入框、拖入区这类"凹下去"的表面。
    pub field: Color32,
    /// 只表示失败，不表示强调。
    pub danger: Color32,
}

/// 界面字号与尺寸，单位是屏幕点。
#[derive(Clone, Debug, PartialEq)]
pub struct Ui {
    pub title: f32,
    pub body: f32,
    pub meta: f32,
    pub empty: f32,
    pub control_height: f32,
    pub radius_control: u8,
    pub radius_surface: u8,
    pub gap_s: f32,
    pub gap_m: f32,
    pub gap_l: f32,
}

/// 一套主题：名字 + 两套值。
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub chrome: Chrome,
    pub ui: Ui,
}

/// 内置浅色。文件里没写的 token 都用这里的值。
pub fn light() -> Theme {
    Theme {
        name: "浅色".to_owned(),
        chrome: Chrome {
            window: Color32::from_rgb(255, 255, 255),
            panel: Color32::from_rgb(250, 250, 251),
            rail: Color32::from_rgb(239, 239, 242),
            line: Color32::from_rgb(232, 232, 236),
            line_soft: Color32::from_rgb(239, 238, 242),
            text: Color32::from_rgb(23, 23, 26),
            text_muted: Color32::from_rgb(78, 78, 87),
            text_faint: Color32::from_rgb(117, 117, 126),
            hover: Color32::from_rgb(240, 240, 243),
            pressed: Color32::from_rgb(231, 231, 235),
            selected: Color32::from_rgb(237, 237, 241),
            field: Color32::from_rgb(241, 241, 244),
            danger: Color32::from_rgb(178, 42, 42),
        },
        ui: Ui {
            title: 14.0,
            body: 13.0,
            meta: 11.5,
            empty: 15.0,
            control_height: 28.0,
            radius_control: 6,
            radius_surface: 10,
            gap_s: 4.0,
            gap_m: 8.0,
            gap_l: 16.0,
        },
    }
}

static CURRENT: OnceLock<Theme> = OnceLock::new();

/// 装上这一套。只在启动时装一次，之后界面到处都是只读。
pub fn install(theme: Theme) -> Result<(), &'static str> {
    CURRENT.set(theme).map_err(|_| "主题只能装一次")
}

/// 当前主题。没装过就是内置浅色。
pub fn current() -> &'static Theme {
    CURRENT.get_or_init(light)
}

pub fn chrome() -> &'static Chrome {
    &current().chrome
}

pub fn ui() -> &'static Ui {
    &current().ui
}

/// 主题文件从哪来。读文件是 app 的事，界面只拿原文。
pub trait ThemeSource {
    /// 主题文件原文；没有文件返回 None。
    fn theme_text(&self) -> Option<String>;
}

impl ThemeSource for () {
    fn theme_text(&self) -> Option<String> {
        None
    }
}

/// 读一个主题：没有文件就用内置浅色；文件写坏了也退回内置浅色，并把原因带回去。
pub fn load(source: &dyn ThemeSource) -> (Theme, Option<String>) {
    let Some(text) = source.theme_text() else {
        return (light(), None);
    };
    match parse(&text) {
        Ok(theme) => (theme, None),
        Err(error) => (light(), Some(error)),
    }
}

/// 解析主题文件。只认要用的那几个 token，多写一个都报错。
pub fn parse(text: &str) -> Result<Theme, String> {
    let root = json::parse(text)?;
    let json::Value::Map(fields) = root else {
        return Err("主题文件的根必须是一个对象".to_owned());
    };
    let mut theme = light();
    if let Some(value) = lookup(&fields, "name") {
        theme.name = as_text(value, "name")?.to_owned();
    }
    match lookup(&fields, "extends") {
        None => {}
        Some(value) => {
            let base = as_text(value, "extends")?;
            if base != "light" {
                return Err(format!("不认识的基础主题：{base}，目前只有 light"));
            }
        }
    }
    let Some(tokens) = lookup(&fields, "tokens") else {
        return Err("缺少 tokens".to_owned());
    };
    let json::Value::Map(tokens) = tokens else {
        return Err("tokens 必须是一个对象".to_owned());
    };
    for (key, value) in tokens {
        apply_token(&mut theme, key, value)?;
    }
    if theme.ui.title <= 0.0 || theme.ui.body <= 0.0 || theme.ui.meta <= 0.0 {
        return Err("字号必须大于 0".to_owned());
    }
    Ok(theme)
}

fn lookup<'a>(fields: &'a [(String, json::Value)], key: &str) -> Option<&'a json::Value> {
    fields
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

fn apply_token(theme: &mut Theme, key: &str, value: &json::Value) -> Result<(), String> {
    let chrome = &mut theme.chrome;
    let ui = &mut theme.ui;
    match key {
        "chrome.window" => chrome.window = colour(value, key)?,
        "chrome.panel" => chrome.panel = colour(value, key)?,
        "chrome.rail" => chrome.rail = colour(value, key)?,
        "chrome.line" => chrome.line = colour(value, key)?,
        "chrome.line_soft" => chrome.line_soft = colour(value, key)?,
        "chrome.text" => chrome.text = colour(value, key)?,
        "chrome.text_muted" => chrome.text_muted = colour(value, key)?,
        "chrome.text_faint" => chrome.text_faint = colour(value, key)?,
        "chrome.hover" => chrome.hover = colour(value, key)?,
        "chrome.pressed" => chrome.pressed = colour(value, key)?,
        "chrome.selected" => chrome.selected = colour(value, key)?,
        "chrome.field" => chrome.field = colour(value, key)?,
        "chrome.danger" => chrome.danger = colour(value, key)?,
        "ui.title" => ui.title = number(value, key)?,
        "ui.body" => ui.body = number(value, key)?,
        "ui.meta" => ui.meta = number(value, key)?,
        "ui.empty" => ui.empty = number(value, key)?,
        "ui.control_height" => ui.control_height = number(value, key)?,
        "ui.radius_control" => ui.radius_control = small_radius(value, key)?,
        "ui.radius_surface" => ui.radius_surface = small_radius(value, key)?,
        "ui.gap_s" => ui.gap_s = number(value, key)?,
        "ui.gap_m" => ui.gap_m = number(value, key)?,
        "ui.gap_l" => ui.gap_l = number(value, key)?,
        other => return Err(format!("不认识的主题 token：{other}")),
    }
    Ok(())
}

fn as_text<'a>(value: &'a json::Value, key: &str) -> Result<&'a str, String> {
    match value {
        json::Value::Text(text) => Ok(text),
        _ => Err(format!("{key} 应该是字符串")),
    }
}

fn colour(value: &json::Value, key: &str) -> Result<Color32, String> {
    let text = as_text(value, key)?;
    let digits = text
        .strip_prefix('#')
        .ok_or_else(|| format!("{key} 的颜色要写成 #RRGGBB，收到 {text}"))?;
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{key} 的颜色要写成 #RRGGBB，收到 {text}"));
    }
    let channel = |at: usize| {
        u8::from_str_radix(&digits[at..at + 2], 16)
            .map_err(|_| format!("{key} 的颜色解析失败：{text}"))
    };
    Ok(Color32::from_rgb(channel(0)?, channel(2)?, channel(4)?))
}

fn number(value: &json::Value, key: &str) -> Result<f32, String> {
    match value {
        json::Value::Number(value) if value.is_finite() && *value > 0.0 => Ok(*value as f32),
        json::Value::Number(_) => Err(format!("{key} 必须是大于 0 的数")),
        _ => Err(format!("{key} 应该是数字")),
    }
}

fn small_radius(value: &json::Value, key: &str) -> Result<u8, String> {
    let value = number(value, key)?;
    if value > 64.0 {
        return Err(format!("{key} 太大，最多 64"));
    }
    Ok(value.round() as u8)
}

/// 把 egui 的默认外观压到当前主题上。视图里不再单独设样式。
pub fn apply(ctx: &egui::Context) {
    let theme = current();
    let chrome = &theme.chrome;
    let ui = &theme.ui;
    let mut style = (*ctx.style_of(egui::Theme::Light)).clone();
    let mut visuals = egui::Visuals::light();
    visuals.override_text_color = Some(chrome.text);
    visuals.panel_fill = chrome.panel;
    visuals.window_fill = chrome.window;
    visuals.extreme_bg_color = chrome.field;
    visuals.faint_bg_color = chrome.hover;
    visuals.window_stroke = Stroke::new(1.0, chrome.line);
    visuals.selection.bg_fill = chrome.selected;
    visuals.selection.stroke = Stroke::new(1.0, chrome.text);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, chrome.line);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, chrome.text_muted);
    visuals.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, chrome.text_muted);
    visuals.widgets.hovered.weak_bg_fill = chrome.hover;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, chrome.text);
    visuals.widgets.active.weak_bg_fill = chrome.pressed;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, chrome.text);
    let radius = CornerRadius::same(ui.radius_control);
    let widgets = &mut visuals.widgets;
    for widget in [
        &mut widgets.noninteractive,
        &mut widgets.inactive,
        &mut widgets.hovered,
        &mut widgets.active,
        &mut widgets.open,
    ] {
        widget.corner_radius = radius;
    }
    style.visuals = visuals;
    // 控件（菜单、输入框）走同一套字号；画布里的字号另算。
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(ui.title),
    );
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(ui.body));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(ui.body));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(ui.meta));
    style.spacing.item_spacing = egui::vec2(ui.gap_m, ui.gap_s);
    style.spacing.button_padding = egui::vec2(ui.gap_m, 4.0);
    style.spacing.interact_size.y = ui.control_height;
    ctx.set_style_of(egui::Theme::Light, style);
}

/// 主题文件要用的那点 JSON：对象、数组、字符串、数字、真假、空。
///
/// 只认这些写法，别的（注释、尾逗号、单引号、NaN）一律报错，不猜。
mod json {
    #[derive(Clone, Debug, PartialEq)]
    pub enum Value {
        Null,
        Bool(bool),
        Number(f64),
        Text(String),
        List(Vec<Value>),
        Map(Vec<(String, Value)>),
    }

    /// 嵌套太深就当文件有问题，别把栈撑爆。
    const MAX_DEPTH: usize = 16;

    pub fn parse(text: &str) -> Result<Value, String> {
        let mut parser = Parser {
            bytes: text.as_bytes(),
            at: 0,
        };
        let value = parser.value(0)?;
        parser.space();
        if parser.at != parser.bytes.len() {
            return Err(format!("第 {} 个字符之后还有多余内容", parser.at + 1));
        }
        Ok(value)
    }

    struct Parser<'a> {
        bytes: &'a [u8],
        at: usize,
    }

    impl Parser<'_> {
        fn peek(&self) -> Option<u8> {
            self.bytes.get(self.at).copied()
        }

        fn bump(&mut self) -> Option<u8> {
            let byte = self.peek();
            if byte.is_some() {
                self.at += 1;
            }
            byte
        }

        fn space(&mut self) {
            while matches!(self.peek(), Some(0x20 | 0x09 | 0x0a | 0x0d)) {
                self.at += 1;
            }
        }

        fn expect(&mut self, byte: u8, what: &str) -> Result<(), String> {
            self.space();
            if self.bump() == Some(byte) {
                Ok(())
            } else {
                Err(format!("第 {} 个字符处应该有一个 {what}", self.at + 1))
            }
        }

        fn literal(&mut self, word: &str) -> Result<(), String> {
            if self.bytes[self.at..].starts_with(word.as_bytes()) {
                self.at += word.len();
                Ok(())
            } else {
                Err(format!("第 {} 个字符处应该是 {word}", self.at + 1))
            }
        }

        fn value(&mut self, depth: usize) -> Result<Value, String> {
            if depth > MAX_DEPTH {
                return Err("嵌套太深".to_owned());
            }
            self.space();
            match self.peek() {
                Some(b'{') => self.map(depth),
                Some(b'[') => self.list(depth),
                Some(b'"') => Ok(Value::Text(self.text()?)),
                Some(b't') => {
                    self.literal("true")?;
                    Ok(Value::Bool(true))
                }
                Some(b'f') => {
                    self.literal("false")?;
                    Ok(Value::Bool(false))
                }
                Some(b'n') => {
                    self.literal("null")?;
                    Ok(Value::Null)
                }
                Some(byte) if byte == b'-' || byte.is_ascii_digit() => self.number(),
                Some(_) => Err(format!("第 {} 个字符不知道该读成什么", self.at + 1)),
                None => Err("文件还没读完就断了".to_owned()),
            }
        }

        fn map(&mut self, depth: usize) -> Result<Value, String> {
            self.expect(b'{', "左花括号")?;
            let mut fields = Vec::new();
            self.space();
            if self.peek() == Some(b'}') {
                self.bump();
                return Ok(Value::Map(fields));
            }
            loop {
                self.space();
                let key = self.text()?;
                self.expect(b':', "冒号")?;
                let value = self.value(depth + 1)?;
                fields.push((key, value));
                self.space();
                match self.bump() {
                    Some(b',') => continue,
                    Some(b'}') => return Ok(Value::Map(fields)),
                    _ => return Err(format!("第 {} 个字符处应该是逗号或右花括号", self.at)),
                }
            }
        }

        fn list(&mut self, depth: usize) -> Result<Value, String> {
            self.expect(b'[', "左方括号")?;
            let mut items = Vec::new();
            self.space();
            if self.peek() == Some(b']') {
                self.bump();
                return Ok(Value::List(items));
            }
            loop {
                items.push(self.value(depth + 1)?);
                self.space();
                match self.bump() {
                    Some(b',') => continue,
                    Some(b']') => return Ok(Value::List(items)),
                    _ => return Err(format!("第 {} 个字符处应该是逗号或右方括号", self.at)),
                }
            }
        }

        fn text(&mut self) -> Result<String, String> {
            self.space();
            if self.bump() != Some(0x22) {
                return Err(format!("第 {} 个字符处应该是字符串", self.at));
            }
            let mut out = String::new();
            loop {
                let Some(byte) = self.bump() else {
                    return Err("字符串没有闭合".to_owned());
                };
                match byte {
                    0x22 => return Ok(out),
                    0x5c => {
                        let Some(escape) = self.bump() else {
                            return Err("转义没写完整".to_owned());
                        };
                        match escape {
                            0x22 => out.push('"'),
                            0x5c => out.push(char::from(0x5c)),
                            0x2f => out.push('/'),
                            0x62 => out.push(char::from(0x08)),
                            0x66 => out.push(char::from(0x0c)),
                            0x6e => out.push(char::from(0x0a)),
                            0x72 => out.push(char::from(0x0d)),
                            0x74 => out.push(char::from(0x09)),
                            0x75 => {
                                let code = self.hex4()?;
                                match char::from_u32(code) {
                                    Some(character) => out.push(character),
                                    None => return Err("反斜杠 u 后面的码点不合法".to_owned()),
                                }
                            }
                            other => {
                                return Err(format!("不认识的转义：{}", other as char));
                            }
                        }
                    }
                    byte if byte < 0x20 => return Err("字符串里有控制字符".to_owned()),
                    byte if byte.is_ascii() => out.push(byte as char),
                    first => {
                        // 非 ASCII 的字按 UTF-8 原样收下。
                        let start = self.at - 1;
                        let width = utf8_width(first);
                        let end = start + width;
                        if end > self.bytes.len() {
                            return Err("字符串里的文字不完整".to_owned());
                        }
                        self.at = end;
                        match std::str::from_utf8(&self.bytes[start..end]) {
                            Ok(text) => out.push_str(text),
                            Err(_) => return Err("字符串里的文字不是 UTF-8".to_owned()),
                        }
                    }
                }
            }
        }

        fn hex4(&mut self) -> Result<u32, String> {
            if self.at + 4 > self.bytes.len() {
                return Err("\\u 后面缺四位十六进制".to_owned());
            }
            let digits = std::str::from_utf8(&self.bytes[self.at..self.at + 4])
                .map_err(|_| "\\u 后面的内容不是十六进制".to_owned())?;
            let code = u32::from_str_radix(digits, 16)
                .map_err(|_| "\\u 后面的内容不是十六进制".to_owned())?;
            self.at += 4;
            Ok(code)
        }

        fn number(&mut self) -> Result<Value, String> {
            let start = self.at;
            if self.peek() == Some(b'-') {
                self.bump();
            }
            while matches!(self.peek(), Some(byte) if byte.is_ascii_digit()) {
                self.bump();
            }
            if self.peek() == Some(b'.') {
                self.bump();
                while matches!(self.peek(), Some(byte) if byte.is_ascii_digit()) {
                    self.bump();
                }
            }
            if matches!(self.peek(), Some(b'e' | b'E')) {
                self.bump();
                if matches!(self.peek(), Some(b'+' | b'-')) {
                    self.bump();
                }
                while matches!(self.peek(), Some(byte) if byte.is_ascii_digit()) {
                    self.bump();
                }
            }
            let text = std::str::from_utf8(&self.bytes[start..self.at])
                .map_err(|_| "数字不是 UTF-8".to_owned())?;
            text.parse::<f64>()
                .map(Value::Number)
                .map_err(|_| format!("读不懂这个数字：{text}"))
        }
    }

    fn utf8_width(first: u8) -> usize {
        match first {
            0x00..=0x7f => 1,
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            _ => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(&'static str);

    impl ThemeSource for Fixed {
        fn theme_text(&self) -> Option<String> {
            Some(self.0.to_owned())
        }
    }

    #[test]
    fn no_file_means_the_built_in_light() {
        let (theme, warning) = load(&());
        assert!(warning.is_none());
        assert_eq!(theme, light());
    }

    #[test]
    fn one_token_can_be_overridden() {
        let theme = parse(r##"{"tokens": {"chrome.window": "#101010"}}"##).expect("要能解析");
        assert_eq!(theme.chrome.window, Color32::from_rgb(0x10, 0x10, 0x10));
        // 没写的继承默认。
        assert_eq!(theme.chrome.panel, light().chrome.panel);
    }

    #[test]
    fn name_and_sizes_are_readable() {
        let theme =
            parse(r##"{"name": "夜", "tokens": {"ui.body": 14, "ui.radius_surface": 12}}"##)
                .expect("要能解析");
        assert_eq!(theme.name, "夜");
        assert_eq!(theme.ui.body, 14.0);
        assert_eq!(theme.ui.radius_surface, 12);
    }

    #[test]
    fn unknown_token_is_an_error() {
        let error = parse(r##"{"tokens": {"chrome.nope": "#ffffff"}}"##).unwrap_err();
        assert!(error.contains("chrome.nope"), "{error}");
    }

    #[test]
    fn a_colour_must_be_six_digits() {
        assert!(parse(r##"{"tokens": {"chrome.text": "#fff"}}"##).is_err());
        assert!(parse(r##"{"tokens": {"chrome.text": "not-a-colour"}}"##).is_err());
    }

    #[test]
    fn a_number_must_be_positive() {
        assert!(parse(r##"{"tokens": {"ui.body": 0}}"##).is_err());
        assert!(parse(r##"{"tokens": {"ui.body": -3}}"##).is_err());
        assert!(parse(r##"{"tokens": {"ui.body": "13"}}"##).is_err());
    }

    #[test]
    fn broken_json_is_reported_not_guessed() {
        assert!(parse("{").is_err());
        assert!(parse(r##"{"tokens": {}}"##).is_ok());
        assert!(parse(r##"{"tokens": {},}"##).is_err());
        assert!(parse(r##"{"tokens": {"ui.body": 13}"##).is_err());
    }

    #[test]
    fn only_light_can_be_extended() {
        assert!(parse(r##"{"extends": "light", "tokens": {}}"##).is_ok());
        assert!(parse(r##"{"extends": "dark", "tokens": {}}"##).is_err());
    }

    #[test]
    fn a_broken_file_falls_back_to_light_with_a_reason() {
        let (theme, warning) = load(&Fixed(r##"{"tokens": {"nope": 1}}"##));
        assert_eq!(theme, light());
        assert!(warning.expect("要带回原因").contains("nope"));
    }

    #[test]
    fn utf8_names_survive_the_parser() {
        let theme = parse(r##"{"name": "浅色·测试", "tokens": {}}"##).expect("要能解析");
        assert_eq!(theme.name, "浅色·测试");
    }
}
