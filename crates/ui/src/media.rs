#![forbid(unsafe_code)]

//! 媒体读取与图片纹理缓存。
//!
//! 界面不直接碰文件：取字节由 app 实现 `MediaSource`，这里只负责解码与缓存纹理。

use std::collections::HashMap;

use egui::{ColorImage, Context, TextureHandle, TextureId, TextureOptions, Vec2};

/// 按内容哈希取媒体原始字节。取不到返回 None，界面画占位。
pub trait MediaSource {
    fn media(&self, hash: &str) -> Option<Vec<u8>>;

    /// 只问大小，不读内容。资源栏要显示体积，但不想为了几个字节去读整张图。
    /// 取不到就返回 None，界面不显示大小，不显示 0。
    fn media_len(&self, _hash: &str) -> Option<u64> {
        None
    }
}

impl MediaSource for () {
    fn media(&self, _hash: &str) -> Option<Vec<u8>> {
        None
    }
}

/// 画一张图片需要的全部信息。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureInfo {
    pub id: TextureId,
    /// 纹理原始尺寸，单位是像素。
    pub size: Vec2,
}

enum Cached {
    Ready {
        /// 持有纹理句柄，缓存条目被丢掉之前纹理才不会被回收。
        _handle: TextureHandle,
        info: TextureInfo,
    },
    Missing,
}

/// 图片纹理缓存。同一张图只解码、上传一次；找不到或解不开的也记下来，别每帧重试。
#[derive(Default)]
pub struct ImageCache {
    images: HashMap<String, Cached>,
}

impl ImageCache {
    /// 取一张图的纹理。取不到或解不开返回 None，调用方画占位。
    pub fn texture(
        &mut self,
        context: &Context,
        hash: &str,
        source: &dyn MediaSource,
    ) -> Option<TextureInfo> {
        if hash.is_empty() {
            return None;
        }
        if let Some(cached) = self.images.get(hash) {
            return match cached {
                Cached::Ready { info, .. } => Some(*info),
                Cached::Missing => None,
            };
        }

        let decoded = source
            .media(hash)
            .and_then(|bytes| decode(context, hash, &bytes));
        let info = decoded.as_ref().map(|(_, info)| *info);
        let cached = match decoded {
            Some((handle, info)) => Cached::Ready {
                _handle: handle,
                info,
            },
            None => Cached::Missing,
        };
        self.images.insert(hash.to_owned(), cached);
        info
    }

    /// 换画板时把纹理丢掉，下次用到再解。
    pub fn clear(&mut self) {
        self.images.clear();
    }
}

/// 解码一张图并上传成纹理。太大的图不接，避免撑爆纹理上限。
fn decode(context: &Context, hash: &str, bytes: &[u8]) -> Option<(TextureHandle, TextureInfo)> {
    let max_side = context.input(|input| input.max_texture_side);
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    if width == 0 || height == 0 || width > max_side || height > max_side {
        return None;
    }
    let colour = ColorImage::from_rgba_unmultiplied([width, height], image.as_raw());
    let handle = context.load_texture(format!("media-{hash}"), colour, TextureOptions::LINEAR);
    let info = TextureInfo {
        id: handle.id(),
        size: handle.size_vec2(),
    };
    Some((handle, info))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(Vec<u8>);

    impl MediaSource for Fixed {
        fn media(&self, hash: &str) -> Option<Vec<u8>> {
            (hash == "known").then(|| self.0.clone())
        }
    }

    #[test]
    fn unknown_media_is_cached_as_missing() {
        let context = Context::default();
        let mut cache = ImageCache::default();
        assert!(cache.texture(&context, "nope", &()).is_none());
        // 第二次仍然拿不到，且不会去问来源。
        assert!(cache.texture(&context, "nope", &()).is_none());
    }

    #[test]
    fn empty_hash_is_not_a_texture() {
        let context = Context::default();
        let mut cache = ImageCache::default();
        assert!(cache.texture(&context, "", &()).is_none());
    }

    #[test]
    fn broken_bytes_fall_back_to_missing() {
        let context = Context::default();
        let source = Fixed(vec![0, 1, 2, 3]);
        let mut cache = ImageCache::default();
        assert!(cache.texture(&context, "known", &source).is_none());
    }

    #[test]
    fn clearing_forgets_every_texture() {
        let context = Context::default();
        let mut cache = ImageCache::default();
        assert!(cache.texture(&context, "nope", &()).is_none());
        cache.clear();
        assert!(cache.images.is_empty());
    }
}
