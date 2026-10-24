//! 系统字体：先列出来，用到哪一族才把它装进 egui。
//!
//! 不随程序带字体文件：那要先选定字体、核对许可、往仓库放几 MB 二进制，
//! 属于发布前的事。这里只做「用系统里已经装好的字体」。
//!
//! 枚举系统字体要几百毫秒，所以整个库是懒加载的：用户第一次打开字体列表，
//! 或者打开的文件里用到了非默认字体时，才会去枚举。

use std::sync::Mutex;

use egui::{
    Context,
    epaint::text::{FontData, FontFamily, FontInsert, FontPriority, InsertFontFamily},
};
use fontique::{
    Collection, CollectionOptions, QueryFamily, QueryStatus, Script, SourceCache,
    SourceCacheOptions,
};

struct State {
    collection: Collection,
    cache: SourceCache,
}

/// 系统字体一览，以及已经装进 egui 的那几族。
pub struct FontLibrary {
    state: Mutex<State>,
    names: Vec<String>,
}

impl FontLibrary {
    /// 枚举一次系统字体。几百毫秒，别在每帧里调。
    pub fn load() -> Self {
        let collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: true,
        });
        let mut state = State {
            collection,
            cache: SourceCache::new(SourceCacheOptions::default()),
        };

        let mut names: Vec<String> = state.collection.family_names().map(str::to_owned).collect();
        names.sort();
        names.dedup();

        Self {
            state: Mutex::new(state),
            names,
        }
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|candidate| candidate == name)
    }

    /// 把第 index 个字体族装进 egui，之后就能用它的名字当字体族。
    pub fn install(&self, context: &Context, index: usize) -> Option<String> {
        let name = self.names.get(index)?.clone();
        let state = &mut *self.state.lock().ok()?;
        let State { collection, cache } = state;

        let mut query = collection.query(cache);
        query.set_families([QueryFamily::Named(name.as_str())]);
        // 让字体族先按简体中文找，找不到再回退。
        query.set_fallbacks(fontique::FallbackKey::new(
            Script::from_bytes(*b"Hani"),
            None,
        ));
        let mut found = None;
        query.matches_with(|font| {
            found = Some(font.clone());
            QueryStatus::Stop
        });
        let font = found?;

        let (blob, _) = font.blob.into_raw_parts();
        // 带内置点阵的字体（宋体最典型）在 epaint 里会画成白色：点阵路径丢了颜色。
        // 选了等于文字消失，所以这里直接不装，让调用方留在原来的字体上。
        let bytes: &[u8] = (*blob).as_ref();
        if has_embedded_bitmaps(bytes) {
            return None;
        }
        context.add_font(FontInsert {
            name: format!("system:{name}"),
            data: FontData::from_blob(blob, font.index),
            families: vec![InsertFontFamily {
                family: FontFamily::Name(name.as_str().into()),
                priority: FontPriority::Highest,
            }],
        });
        Some(name)
    }
}

/// 字体文件里有没有内置点阵表（EBDT / CBDT / sbix）。
///
/// 这几个表在 epaint 的渲染路径上会丢颜色，装上就是白字。查表目录就够，
/// 不必解析字体结构。
fn has_embedded_bitmaps(blob: &[u8]) -> bool {
    [b"EBDT", b"CBDT", b"sbix"].iter().any(|tag| {
        blob.windows(tag.len())
            .take(4096)
            .any(|window| window == *tag)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_library_lists_system_fonts() {
        let library = FontLibrary::load();
        // 机器上总得装了几个字体，没有的话这个用例本来就该红。
        assert!(!library.names().is_empty());
    }

    #[test]
    fn installed_font_can_be_found_by_name() {
        let library = FontLibrary::load();
        let first = library
            .names()
            .first()
            .expect("at least one family")
            .clone();
        assert_eq!(library.index_of(&first), Some(0));
        assert_eq!(library.index_of("这个字体族一定不存在"), None);
    }
}
