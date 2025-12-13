use euclid::{Box2D, Point2D};

use crate::{
    font_storage::FontStorage,
    text::{GlyphPosition, TextLayout},
};

mod glyph_cache;
pub use glyph_cache::{CacheAtlas, GpuCache, GpuCacheConfig, GpuCacheItem};

/// Describes an update to a texture in the atlas.
pub struct AtlasUpdate {
    /// Index of the texture in the atlas array to update.
    pub texture_index: usize,
    /// X coordinate of the update region.
    pub x: usize,
    /// Y coordinate of the update region.
    pub y: usize,
    /// Width of the update region.
    pub width: usize,
    /// Height of the update region.
    pub height: usize,
    /// Bitmap data to upload (row-major).
    pub pixels: Vec<u8>,
}

/// Describes a glyph instance to be drawn.
pub struct GlyphInstance<T> {
    /// Index of the texture in the atlas array.
    pub texture_index: usize,
    /// UV coordinates in the texture atlas.
    pub uv_rect: Box2D<f32, euclid::UnknownUnit>,
    /// Screen coordinates where the glyph should be drawn.
    pub screen_rect: Box2D<f32, euclid::UnknownUnit>,
    /// User data associated with this glyph.
    pub user_data: T,
}

/// Describes a standalone large glyph to be drawn separately.
pub struct StandaloneGlyph<T> {
    /// Width of the glyph image.
    pub width: usize,
    /// Height of the glyph image.
    pub height: usize,
    /// Bitmap data of the glyph.
    pub pixels: Vec<u8>,
    /// Screen coordinates where the glyph should be drawn.
    pub screen_rect: Box2D<f32, euclid::UnknownUnit>,
    /// User data associated with this glyph.
    pub user_data: T,
}

/// Generic GPU renderer that manages an atlas and produces draw commands.
///
/// This renderer does not depend on a specific graphics API. Instead, it calculates
/// atlas updates and instance data, which are passed to callbacks for the actual
/// API-specific rendering (e.g., wgpu).
pub struct GpuRenderer {
    cache: GpuCache,
}

impl GpuRenderer {
    /// Creates a new GPU renderer with the provided cache configuration.
    pub fn new(configs: &[GpuCacheConfig]) -> Self {
        Self {
            cache: GpuCache::new(configs),
        }
    }

    /// Clears the cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Renders the layout, producing atlas updates and draw calls via callbacks.
    ///
    /// This method is for infallible callbacks. Use `try_render` for fallible callbacks.
    pub fn render<T: Clone + Copy>(
        &mut self,
        layout: &TextLayout<T>,
        font_storage: &mut FontStorage,
        mut update_atlas: impl FnMut(&[AtlasUpdate]),
        mut draw_instances: impl FnMut(&[GlyphInstance<T>]),
        mut draw_standalone: impl FnMut(&StandaloneGlyph<T>),
    ) {
        let _: Result<(), ()> = self.try_render(
            layout,
            font_storage,
            &mut |u| {
                update_atlas(u);
                Ok(())
            },
            &mut |i| {
                draw_instances(i);
                Ok(())
            },
            &mut |s| {
                draw_standalone(s);
                Ok(())
            },
        );
    }

    /// Renders the layout, producing atlas updates and draw calls via callbacks.
    ///
    /// This method allows callbacks to return errors, which will be propagated.
    pub fn try_render<T: Clone + Copy, E>(
        &mut self,
        layout: &TextLayout<T>,
        font_storage: &mut FontStorage,
        update_atlas: &mut impl FnMut(&[AtlasUpdate]) -> Result<(), E>,
        draw_instances: &mut impl FnMut(&[GlyphInstance<T>]) -> Result<(), E>,
        draw_standalone: &mut impl FnMut(&StandaloneGlyph<T>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut update_atlas_list: Vec<AtlasUpdate> = Vec::new();
        let mut instance_list: Vec<GlyphInstance<T>> = Vec::new();

        for line in &layout.lines {
            'glyph_loop: for glyph in &line.glyphs {
                let GlyphPosition::<T> {
                    glyph_id,
                    x,
                    y,
                    user_data,
                } = glyph;
                let Some(font) = font_storage.font(glyph_id.font_id()) else {
                    continue 'glyph_loop;
                };
                let metrics = font.metrics_indexed(glyph_id.glyph_index(), glyph_id.font_size());

                let (
                    GpuCacheItem {
                        texture_index,
                        texture_size,
                        glyph_box,
                    },
                    get_or_push_result,
                ) = match self.cache.get_or_push_and_protect(glyph_id, font_storage) {
                    Some(glyph_cache_item) => glyph_cache_item,
                    None => {
                        // upload all new glyph data to atlas
                        if !update_atlas_list.is_empty() {
                            update_atlas(&update_atlas_list)?;
                            update_atlas_list.clear();
                        }

                        // draw call
                        if !instance_list.is_empty() {
                            draw_instances(&instance_list)?;
                            instance_list.clear();
                        }

                        self.cache.new_batch();
                        let Some(glyph_cache_item) =
                            self.cache.get_or_push_and_protect(glyph_id, font_storage)
                        else {
                            let (metrics, glyph_data) = font
                                .rasterize_indexed(glyph_id.glyph_index(), glyph_id.font_size());

                            let isolate = StandaloneGlyph {
                                width: metrics.width,
                                height: metrics.height,
                                pixels: glyph_data,
                                screen_rect: Box2D::new(
                                    Point2D::new(*x, *y),
                                    Point2D::new(
                                        *x + metrics.width as f32,
                                        *y + metrics.height as f32,
                                    ),
                                ),
                                user_data: *user_data,
                            };

                            draw_standalone(&isolate)?;

                            continue 'glyph_loop;
                        };

                        glyph_cache_item
                    }
                };

                let uv_rect = Box2D::new(
                    Point2D::new(
                        glyph_box.min.x as f32 / texture_size as f32,
                        glyph_box.min.y as f32 / texture_size as f32,
                    ),
                    Point2D::new(
                        glyph_box.max.x as f32 / texture_size as f32,
                        glyph_box.max.y as f32 / texture_size as f32,
                    ),
                );

                let screen_rect = Box2D::new(
                    Point2D::new(*x, *y),
                    Point2D::new(*x + metrics.width as f32, *y + metrics.height as f32),
                );

                let glyph_instance = GlyphInstance {
                    texture_index,
                    uv_rect,
                    screen_rect,
                    user_data: *user_data,
                };

                instance_list.push(glyph_instance);

                if let glyph_cache::GetOrPushResult::NeedToUpload = get_or_push_result {
                    let (_, glyph_data) =
                        font.rasterize_indexed(glyph_id.glyph_index(), glyph_id.font_size());

                    update_atlas_list.push(AtlasUpdate {
                        texture_index,
                        x: glyph_box.min.x,
                        y: glyph_box.min.y,
                        width: glyph_box.width(),
                        height: glyph_box.height(),
                        pixels: glyph_data,
                    });
                }
            }
        }

        if !update_atlas_list.is_empty() {
            update_atlas(&update_atlas_list)?;
        }

        if !instance_list.is_empty() {
            draw_instances(&instance_list)?;
        }

        Ok(())
    }
}
