use euclid::Box2D;
use std::collections::HashSet;

use crate::{
    font_storage::FontStorage,
    text::{GlyphPosition, TextLayout},
};

mod glyph_cache;
pub use glyph_cache::{CacheAtlas, GlyphAtlasConfig, GlyphCache, GlyphCacheItem};

pub struct WriteToAtlas {
    atlas_page: usize,
    origin_x: usize,
    origin_y: usize,
    width: usize,
    height: usize,
    data: Vec<u8>,
}

pub struct GlyphInstance<T> {
    atlas_page: usize,
    uv_box: Box2D<f32, euclid::UnknownUnit>,
    position_box: Box2D<f32, euclid::UnknownUnit>,
    user_data: T,
}

pub struct GpuRenderer {
    cache: GlyphCache,
}

impl GpuRenderer {
    pub fn new(configs: Vec<GlyphAtlasConfig>) -> Self {
        Self {
            cache: GlyphCache::new(configs),
        }
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    pub fn render<T>(
        &mut self,
        layout: &TextLayout<T>,
        font_storage: &mut FontStorage,
        mut write_atlas: &mut impl FnMut(Vec<WriteToAtlas>),
        mut draw_call: &mut impl FnMut(Vec<GlyphInstance<T>>),
    ) {
        let update_atlas_list: Vec<WriteToAtlas> = Vec::new();
        let instance_list: Vec<GlyphInstance<T>> = Vec::new();

        for line in &layout.lines {
            for glyph in &line.glyphs {
                let GlyphPosition::<T> {
                    glyph_id,
                    x,
                    y,
                    user_data,
                } = glyph;

                if let Some(glyph_cache_item) =
                    self.cache.get_or_push_and_protect(glyph_id, font_storage)
                {
                    let GlyphCacheItem {
                        atlas_idx,
                        texture_size,
                        glyph_box,
                    } = glyph_cache_item;

                    let glyph_instance = GlyphInstance {
                        atlas_page: atlas_idx,
                        uv_box: todo!(),
                        position_box: todo!(),
                        user_data,
                    };
                } else {
                    todo!();

                    self.cache.new_batch();
                }
            }
        }
    }
}
