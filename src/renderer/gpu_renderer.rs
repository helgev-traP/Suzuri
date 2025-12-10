use euclid::{Box2D, Point2D};

use crate::{
    font_storage::FontStorage,
    text::{GlyphPosition, TextLayout},
};

mod glyph_cache;
pub use glyph_cache::{CacheAtlas, GpuCache, GpuCacheConfig, GpuCacheItem};

pub struct AtlasUpdate {
    pub texture_index: usize,
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

pub struct GlyphInstance<T> {
    pub texture_index: usize,
    pub uv_rect: Box2D<f32, euclid::UnknownUnit>,
    pub screen_rect: Box2D<f32, euclid::UnknownUnit>,
    pub user_data: T,
}

pub struct StandaloneGlyph<T> {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub screen_rect: Box2D<f32, euclid::UnknownUnit>,
    pub user_data: T,
}

pub struct GpuRenderer {
    cache: GpuCache,
}

impl GpuRenderer {
    pub fn new(configs: &[GpuCacheConfig]) -> Self {
        Self {
            cache: GpuCache::new(configs),
        }
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    pub fn render<T: Clone + Copy>(
        &mut self,
        layout: &TextLayout<T>,
        font_storage: &mut FontStorage,
        update_atlas: &mut impl FnMut(&[AtlasUpdate]),
        draw_instances: &mut impl FnMut(&[GlyphInstance<T>]),
        draw_standalone: &mut impl FnMut(&StandaloneGlyph<T>),
    ) {
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
                            update_atlas(&update_atlas_list);
                            update_atlas_list.clear();
                        }

                        // draw call
                        if !instance_list.is_empty() {
                            draw_instances(&instance_list);
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

                            draw_standalone(&isolate);

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
            update_atlas(&update_atlas_list);
        }

        if !instance_list.is_empty() {
            draw_instances(&instance_list);
        }
    }
}
