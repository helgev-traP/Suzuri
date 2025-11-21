mod glyph_cache;

use crate::font_storage::FontStorage;
use crate::renderer::Bitmap;
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

    /// Renders the provided [`TextLayout`] into an [`Bitmap`].
    pub fn render_layout(
        &mut self,
        layout: &TextLayout,
        image_size: [usize; 2],
        font_storage: &mut FontStorage,
    ) -> Bitmap {
        let width = image_size[0];
        let height = image_size[1];

        if width == 0 || height == 0 {
            return Bitmap::new(0, 0);
        }

        let mut bitmap = Bitmap::new(width, height);
        for line in &layout.lines {
            for glyph in &line.glyphs {
                self.render_glyph_into_bitmap(&mut bitmap, glyph, font_storage);
            }
        }

        bitmap
    }

    fn render_glyph_into_bitmap(
        &mut self,
        bitmap: &mut Bitmap,
        glyph_pos: &GlyphPosition,
        font_storage: &mut FontStorage,
    ) {
        let Some(cached) = self.cache.get(&glyph_pos.glyph_id, font_storage) else {
            return;
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
            if iy < 0 || iy as usize >= bitmap.height {
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
                if ix < 0 {
                    continue;
                }

                // Use the shared accumulate method which handles bounds checking (again) and saturation.
                // Double bounds checking is acceptable here for code reuse and safety.
                bitmap.accumulate(ix as usize, iy as usize, src_alpha);
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
