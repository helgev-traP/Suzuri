use crate::font_storage::FontStorage;
use crate::glyph_id::GlyphId;
use crate::text::{GlyphPosition, TextLayout};
use mini_moka::sync::Cache;
use std::sync::Arc;

/// Simple L8 bitmap produced by the [`CpuRenderer`].
///
/// Pixels are arranged in row-major order with the origin at the top-left.
/// Each pixel stores a single 8-bit coverage value where `0` represents
/// transparent/empty and `255` is fully opaque.
pub struct CpuBitmap {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl CpuBitmap {
    fn new(width: usize, height: usize) -> Self {
        let len = width.saturating_mul(height);
        Self {
            width,
            height,
            pixels: vec![0; len],
        }
    }
}

struct CachedGlyph {
    width: usize,
    height: usize,
    data: Vec<u8>,
}

impl CachedGlyph {
    fn weight(&self) -> u32 {
        self.data.len().min(u32::MAX as usize).try_into().unwrap()
    }
}

/// CPU-based renderer that rasterizes glyphs using `fontdue` and caches
/// intermediate bitmaps in an LRU-ish weighted cache.
pub struct CpuRenderer {
    glyph_cache: Cache<GlyphId, Arc<CachedGlyph>>,
}

impl CpuRenderer {
    /// Creates a new renderer with a glyph cache limited by `cache_capacity_bytes`.
    pub fn new(cache_capacity_bytes: u64) -> Self {
        let capacity = cache_capacity_bytes.max(1);
        let glyph_cache = Cache::builder()
            .max_capacity(capacity)
            .weigher(|_, glyph: &Arc<CachedGlyph>| glyph.weight())
            .build();

        Self { glyph_cache }
    }

    /// Renders the provided [`TextLayout`] into an [`CpuBitmap`].
    pub fn render_layout(
        &self,
        layout: &TextLayout,
        image_size: [usize; 2],
        font_storage: &mut FontStorage,
    ) -> CpuBitmap {
        let width = image_size[0];
        let height = image_size[1];

        if width == 0 || height == 0 {
            return CpuBitmap::new(0, 0);
        }

        let mut bitmap = CpuBitmap::new(width, height);
        for line in &layout.lines {
            for glyph in &line.glyphs {
                self.render_glyph_into_bitmap(&mut bitmap, glyph, font_storage);
            }
        }

        bitmap
    }

    fn render_glyph_into_bitmap(
        &self,
        bitmap: &mut CpuBitmap,
        glyph_pos: &GlyphPosition,
        font_storage: &mut FontStorage,
    ) {
        let Some(cached) = self.cached_glyph(glyph_pos.glyph_id, font_storage) else {
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
                if ix < 0 || ix as usize >= bitmap.width {
                    continue;
                }

                let idx = iy as usize * bitmap.width + ix as usize;
                let existing = bitmap.pixels[idx] as u16;
                let combined = existing.saturating_add(src_alpha as u16).min(255);
                bitmap.pixels[idx] = combined as u8;
            }
        }
    }

    fn cached_glyph(
        &self,
        glyph_id: GlyphId,
        font_storage: &mut FontStorage,
    ) -> Option<Arc<CachedGlyph>> {
        if let Some(glyph) = self.glyph_cache.get(&glyph_id) {
            return Some(glyph);
        }

        let font = font_storage.font(glyph_id.font_id())?;
        let (metrics, coverage) =
            font.rasterize_indexed(glyph_id.glyph_index(), glyph_id.font_size());

        if metrics.width == 0 || metrics.height == 0 {
            return None;
        }

        let cached = Arc::new(CachedGlyph {
            width: metrics.width as usize,
            height: metrics.height as usize,
            data: coverage,
        });

        self.glyph_cache.insert(glyph_id, cached.clone());
        Some(cached)
    }
}
