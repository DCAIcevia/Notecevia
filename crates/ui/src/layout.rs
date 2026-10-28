#![forbid(unsafe_code)]

//! 两侧面板的宽度：拖过就记住，下次打开还是这样。
//!
//! 宽度不属于主题：主题管配色与字号，面板宽度是用户自己摆的位置，所以单独存一份。
//! 文件是工作区根目录的 ui.json；读不懂就退回默认，不拦启动。

use crate::theme::json::{self, Value};

/// 左栏的默认宽。
pub const LIST_DEFAULT: f32 = 264.0;
/// 左栏能停下来的范围。
pub const LIST_MIN: f32 = 180.0;
pub const LIST_MAX: f32 = 420.0;
/// 右栏的默认宽。
pub const ASSETS_DEFAULT: f32 = 280.0;
/// 右栏能停下来的范围。
pub const ASSETS_MIN: f32 = 200.0;
pub const ASSETS_MAX: f32 = 480.0;

/// 两条面板的宽度，单位是屏幕点。0 表示收起。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub list: f32,
    pub assets: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            list: LIST_DEFAULT,
            assets: ASSETS_DEFAULT,
        }
    }
}

impl Layout {
    pub fn list_open(&self) -> bool {
        self.list > 0.0
    }

    pub fn assets_open(&self) -> bool {
        self.assets > 0.0
    }

    /// 落定：收起还是收起，展开的宽度对齐到整点并夹在范围里。
    ///
    /// 拖动中宽度跟着指针走，松手之后只允许两种状态：收起，或者落在一个固定范围里。
    pub fn settle(&mut self) {
        self.list = settle_one(self.list, LIST_MIN, LIST_MAX);
        self.assets = settle_one(self.assets, ASSETS_MIN, ASSETS_MAX);
    }

    /// 写回文件的文本。两个整数，不折腾格式，也不写注释。
    pub fn to_json(&self) -> String {
        let quote = '"';
        let mut text = String::from("{\n");
        text.push_str(&format!(
            "  {quote}list{quote}: {},\n",
            self.list.round() as i64
        ));
        text.push_str(&format!(
            "  {quote}assets{quote}: {}\n",
            self.assets.round() as i64
        ));
        text.push('}');
        text.push('\n');
        text
    }

    /// 读文件：只认 list 与 assets 两个数，多余的键不管。
    ///
    /// 少了哪个键就用那个键的默认；数不对就报错，由调用方退回默认整套。
    pub fn parse(text: &str) -> Result<Self, String> {
        let Value::Map(fields) = json::parse(text)? else {
            return Err("最外层必须是对象".to_owned());
        };
        let mut layout = Self::default();
        if let Some(value) = lookup(&fields, "list") {
            layout.list = width(value, "list")?;
        }
        if let Some(value) = lookup(&fields, "assets") {
            layout.assets = width(value, "assets")?;
        }
        layout.settle();
        Ok(layout)
    }
}

/// 主区最少要留的宽度：再窄就没法看内容，宁可把两条栏收起来。
const MAIN_MIN: f32 = 260.0;
/// 拖开一条收起的栏要超过这么远才算真要打开：手一抖不该把栏弹出来。
const REOPEN_AT: f32 = 24.0;
/// 一条展开的栏要比最小宽度再窄这么多，松手才算拖到底收起。
///
/// 拖动过程中宽度只压到最小宽度为止，所以面板不会在手里突然消失。
const COLLAPSE_PAST: f32 = 60.0;

/// 拖动中的宽度：只跟指针走，不会中途跳变。
///
/// 展开的栏压到最小宽度就停住，再往回收也不会消失；收起的栏要拉过 REOPEN_AT
/// 才显形，显形就落在最小宽度上。
pub fn dragging(start: f32, want: f32, min: f32, max: f32) -> f32 {
    if start <= 0.0 {
        if want < REOPEN_AT {
            0.0
        } else {
            want.clamp(min, max)
        }
    } else {
        want.clamp(min, max)
    }
}

/// 松手之后落在哪：只有确实拖过头才收起，其余都落在固定范围里。
pub fn released(start: f32, want: f32, min: f32, max: f32) -> f32 {
    if start > 0.0 && want < min - COLLAPSE_PAST {
        0.0
    } else {
        dragging(start, want, min, max)
    }
}

/// 按可用宽度收一收，返回这一刻该画多宽。
///
/// 窗口窄到放不下两条栏时先压右栏，再压左栏；压不到最小宽度就把那条收起来——
/// 不能出现一正一负的宽度，那会让主区翻过来。存的值不动：窗口放大回来还是用户摆的宽度。
pub fn fit(list: f32, assets: f32, available: f32) -> (f32, f32) {
    // 两条栏加起来最多用掉"可用宽度减去主区下限"：主区窄到看不清内容就不算布局。
    let available = (available - MAIN_MIN).max(0.0);
    if list + assets <= available {
        return (list, assets);
    }
    let assets = fit_one(assets, (available - list).max(0.0), ASSETS_MIN);
    let list = fit_one(list, (available - assets).max(0.0), LIST_MIN);
    (list, assets)
}

/// 一条栏在剩下的空间里该画多宽：放不到最小宽度就干脆收起。
fn fit_one(width: f32, room: f32, min: f32) -> f32 {
    if width <= 0.0 || room < min {
        0.0
    } else {
        width.min(room)
    }
}

/// 落定一条栏的宽度：收起是收起，展开就得落在范围里。
fn settle_one(width: f32, min: f32, max: f32) -> f32 {
    if !width.is_finite() || width <= 0.0 {
        0.0
    } else {
        width.round().clamp(min, max)
    }
}

fn lookup<'a>(fields: &'a [(String, Value)], key: &str) -> Option<&'a Value> {
    fields
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

fn width(value: &Value, key: &str) -> Result<f32, String> {
    match value {
        Value::Number(number) if number.is_finite() && *number >= 0.0 => Ok(*number as f32),
        Value::Number(_) => Err(format!("{key} 不能是负数，也不能不是数")),
        _ => Err(format!("{key} 必须是数字")),
    }
}

/// 布局文件从哪来。读文件是 app 的事，界面只拿原文。
pub trait LayoutSource {
    /// 布局文件原文；没有文件返回 None。
    fn layout_text(&self) -> Option<String>;
}

impl LayoutSource for () {
    fn layout_text(&self) -> Option<String> {
        None
    }
}

/// 读一次布局。文件坏了说一句，界面照默认起。
pub fn load(source: &dyn LayoutSource) -> (Layout, Option<String>) {
    let Some(text) = source.layout_text() else {
        return (Layout::default(), None);
    };
    match Layout::parse(&text) {
        Ok(layout) => (layout, None),
        Err(error) => (Layout::default(), Some(error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_round_trips() {
        let layout = Layout::default();
        assert_eq!(Layout::parse(&layout.to_json()), Ok(layout));
    }

    #[test]
    fn collapsed_stays_collapsed() {
        let layout = Layout {
            list: 0.0,
            assets: 0.0,
        };
        let back = Layout::parse(&layout.to_json()).expect("读回来");
        assert!(!back.list_open());
        assert!(!back.assets_open());
    }

    #[test]
    fn out_of_range_is_clamped_to_the_range() {
        let back = Layout::parse(r#"{"list": 9999, "assets": 10}"#).expect("读回来");
        assert_eq!(back.list, LIST_MAX);
        assert_eq!(back.assets, ASSETS_MIN);
    }

    #[test]
    fn missing_key_keeps_the_default() {
        let back = Layout::parse(r#"{"list": 300}"#).expect("读回来");
        assert_eq!(back.list, 300.0);
        assert_eq!(back.assets, ASSETS_DEFAULT);
    }

    #[test]
    fn broken_json_is_an_error_not_a_guess() {
        assert!(Layout::parse("{").is_err());
        assert!(Layout::parse(r#"{"list": "宽"}"#).is_err());
        assert!(Layout::parse("[]").is_err());
    }

    #[test]
    fn dragging_never_snaps_the_panel_shut() {
        // 展开的栏一路往回收：宽度停在最小宽度，不会在手里突然跳成收起。
        assert_eq!(
            dragging(LIST_DEFAULT, LIST_DEFAULT, LIST_MIN, LIST_MAX),
            LIST_DEFAULT
        );
        assert_eq!(
            dragging(LIST_DEFAULT, LIST_DEFAULT - 60.0, LIST_MIN, LIST_MAX),
            LIST_DEFAULT - 60.0,
            "中间跟着指针走"
        );
        assert_eq!(
            dragging(LIST_DEFAULT, LIST_MIN - 1.0, LIST_MIN, LIST_MAX),
            LIST_MIN,
            "拖到最小宽度就停住"
        );
        assert_eq!(
            dragging(LIST_DEFAULT, -500.0, LIST_MIN, LIST_MAX),
            LIST_MIN,
            "拖过头也还是最小宽度"
        );
    }

    #[test]
    fn a_collapsed_panel_opens_after_a_deliberate_pull() {
        assert_eq!(
            dragging(0.0, 10.0, LIST_MIN, LIST_MAX),
            0.0,
            "手一抖不该把栏弹出来"
        );
        assert_eq!(dragging(0.0, 40.0, LIST_MIN, LIST_MAX), LIST_MIN);
    }

    #[test]
    fn release_collapses_only_when_dragged_well_past_the_minimum() {
        assert_eq!(
            released(LIST_DEFAULT, LIST_MIN - 1.0, LIST_MIN, LIST_MAX),
            LIST_MIN,
            "只压过一点点，松手还是最小宽度"
        );
        assert_eq!(
            released(LIST_DEFAULT, LIST_MIN - 61.0, LIST_MIN, LIST_MAX),
            0.0,
            "确实往里拖了一截，松手才收起"
        );
        assert_eq!(released(LIST_DEFAULT, 120.0, LIST_MIN, LIST_MAX), LIST_MIN);
        assert_eq!(released(0.0, 10.0, LIST_MIN, LIST_MAX), 0.0);
    }

    #[test]
    fn fit_keeps_the_layout_when_there_is_room() {
        assert_eq!(
            fit(LIST_DEFAULT, ASSETS_DEFAULT, 960.0),
            (LIST_DEFAULT, ASSETS_DEFAULT)
        );
    }

    #[test]
    fn fit_takes_the_right_panel_first() {
        // 主区留够 260 之后还剩 500：右栏压到放得下为止，左栏不动。
        let (list, assets) = fit(LIST_DEFAULT, ASSETS_DEFAULT, 760.0);
        assert_eq!(list, LIST_DEFAULT);
        assert_eq!(assets, 500.0 - LIST_DEFAULT);
    }

    #[test]
    fn fit_collapses_what_does_not_fit() {
        let (list, assets) = fit(LIST_DEFAULT, ASSETS_DEFAULT, 460.0);
        assert_eq!(assets, 0.0, "右栏先收起来");
        assert_eq!(list, 200.0, "左栏还剩多少就画多少");
        let (list, assets) = fit(LIST_DEFAULT, ASSETS_DEFAULT, 40.0);
        assert_eq!(
            (list, assets),
            (0.0, 0.0),
            "两条都放不下就都收起，主区不能翻过来"
        );
    }

    #[test]
    fn fit_always_leaves_room_for_the_main_area() {
        // 一侧 1280 宽：两条栏按存的宽度摆，主区够用。
        assert_eq!(fit(LIST_MAX, ASSETS_MAX, 1228.0), (LIST_MAX, ASSETS_MAX));
        // 缩到 720：两条栏都得让位，主区仍然留得住 260。
        let (list, assets) = fit(LIST_MAX, ASSETS_MAX, 668.0);
        assert!(list + assets <= 668.0 - MAIN_MIN, "不能把主区挤没");
    }
}
