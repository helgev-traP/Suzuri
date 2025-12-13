use crate::font_storage::FontStorage;
use crate::text::{GlyphPosition, TextLayout};

mod glyph_cache;
pub use glyph_cache::{CpuCache, CpuCacheConfig, CpuCacheItem};

/// CPU-based text renderer.
///
/// ## Overview
///
/// `CpuRenderer` rasterizes glyphs into a CPU-side cache and renders them into a
/// provided pixel buffer (e.g., a `Vec<u8>`). It is useful for software rendering
/// contexts, generating initial textures for other engines (like Unity), or debug visualizations.
///
/// It uses a Least Recently Used (LRU) cache policy to manage rasterized glyph bitmaps efficiently.
///
/// ## Integration
///
/// This component can be used in two ways:
/// -   **Through [`crate::FontSystem`]**: Provides a high-level API where `FontSystem` manages the renderer instance.
/// -   **Standalone**: You can instantiate and use this renderer directly. This offers more granular control over resource management and rendering.
///
/// ## Usage
///
/// ```rust,no_run
/// use suzuri::{
///     FontSystem, fontdb,
///     renderer::CpuCacheConfig,
///     text::{TextData, TextElement, TextLayoutConfig}
/// };
/// use std::num::NonZeroUsize;
///
/// let font_system = FontSystem::new();
/// font_system.load_system_fonts();
///
/// // 1. Initialize Renderer
/// let cache_configs = [
///     CpuCacheConfig {
///         block_size: NonZeroUsize::new(32 * 32).unwrap(), // width * height
///         capacity: NonZeroUsize::new(1024).unwrap(),
///     },
/// ];
/// font_system.cpu_init(&cache_configs);
///
/// // 2. Layout Text
/// let mut data = TextData::<()>::new();
/// // ... (append text elements) ...
/// let layout = font_system.layout_text(&data, &TextLayoutConfig::default());
///
/// // 3. Render
/// let width = 640;
/// let height = 480;
/// let mut screen_buffer = vec![0u8; width * height];
///
/// font_system.cpu_render(
///     &layout,
///     [width, height],
///     &mut |[x, y], alpha, user_data| {
///         let idx = y * width + x;
///         // Simple alpha blending
///         screen_buffer[idx] = screen_buffer[idx].saturating_add(alpha);
///     }
/// );
/// ```
pub struct CpuRenderer {
    cache: CpuCache,
}

impl CpuRenderer {
    /// Creates a renderer from the provided cache.
    pub fn new(configs: &[CpuCacheConfig]) -> Self {
        Self {
            cache: CpuCache::new(configs),
        }
    }

    /// Clears the renderer's cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
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
                CpuCacheItem {
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
}
