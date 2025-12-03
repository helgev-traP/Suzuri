mod glyph_cache;

use crate::font_storage::FontStorage;
use crate::text::{GlyphPosition, TextLayout};

pub use glyph_cache::{GlyphCache, GlyphCacheItem};

/// CPU-based renderer that rasterizes glyphs using a cache.
pub struct CpuRenderer {
    cache: GlyphCache,
}

impl CpuRenderer {
    /// Creates a renderer from the provided cache.
    pub fn new(cache: GlyphCache) -> Self {
        Self { cache }
    }

    /// Renders the provided [`TextLayout`] by calling the closure for each pixel.
    pub fn render<T>(
        &mut self,
        layout: &TextLayout<T>,
        image_size: [usize; 2],
        font_storage: &mut FontStorage,
        f: &mut dyn FnMut([usize; 2], u8, &T),
    ) {
        let width = image_size[0];
        let height = image_size[1];

        if width == 0 || height == 0 {
            return;
        }

        for line in &layout.lines {
            if line.bottom <= 0.0 || line.top >= height as f32 {
                continue;
            }
            for glyph in &line.glyphs {
                self.render_glyph(glyph, font_storage, image_size, f);
            }
        }
    }

    fn render_glyph<T>(
        &mut self,
        glyph_pos: &GlyphPosition<T>,
        font_storage: &mut FontStorage,
        image_size: [usize; 2],
        f: &mut dyn FnMut([usize; 2], u8, &T),
    ) {
        let cached = match self.cache.get(&glyph_pos.glyph_id, font_storage) {
            Some(cached) => cached,
            None => {
                let Some(font) = font_storage.font(glyph_pos.glyph_id.font_id()) else {
                    return;
                };
                let (metrics, bitmap) = font.rasterize_indexed(
                    glyph_pos.glyph_id.glyph_index(),
                    glyph_pos.glyph_id.font_size(),
                );
                GlyphCacheItem {
                    width: metrics.width,
                    height: metrics.height,
                    data: std::borrow::Cow::Owned(bitmap),
                }
            }
        };

        if cached.width == 0 || cached.height == 0 {
            return;
        }

        let glyph_width = cached.width;
        let glyph_height = cached.height;
        let origin_x = glyph_pos.x;
        let origin_y = glyph_pos.y;

        for row in 0..glyph_height {
            let y = origin_y + row as f32;
            if y < 0.0 {
                continue;
            }
            let iy = y.floor() as isize;
            if iy < 0 || iy as usize >= image_size[1] {
                continue;
            }

            for col in 0..glyph_width {
                let src_alpha = cached.data[row * glyph_width + col];
                if src_alpha == 0 {
                    continue;
                }

                let x = origin_x + col as f32;
                if x < 0.0 {
                    continue;
                }

                let ix = x.floor() as isize;
                if ix < 0 || ix as usize >= image_size[0] {
                    continue;
                }

                // Use the shared accumulate method which handles bounds checking (again) and saturation.
                // Double bounds checking is acceptable here for code reuse and safety.
                f([ix as usize, iy as usize], src_alpha, &glyph_pos.user_data);
            }
        }
    }

    /// Returns a reference to the underlying glyph cache.
    pub fn cache(&self) -> &GlyphCache {
        &self.cache
    }

    /// Returns a mutable reference to the underlying glyph cache.
    pub fn cache_mut(&mut self) -> &mut GlyphCache {
        &mut self.cache
    }
}
