use crate::{
    font_storage::FontStorage,
    renderer::gpu_renderer::{
        AtlasUpdate, GlyphInstance, GpuCacheConfig, GpuRenderer, StandaloneGlyph,
    },
    text::TextLayout,
};

/// A CPU-based debug renderer that emulates GPU atlas rendering.
///
/// This renderer uses the same atlas and caching logic as `GpuRenderer` but
/// performs all rendering on the CPU. It is useful for debugging and testing
/// the GPU rendering pipeline without requiring actual GPU access.
pub struct CpuDebugRenderer {
    gpu_renderer: GpuRenderer,
    atlases: std::cell::RefCell<Vec<Vec<u8>>>, // List of atlas textures (grayscale)
    atlas_configs: Vec<GpuCacheConfig>,
}

impl CpuDebugRenderer {
    /// Creates a new debug renderer with the given cache configuration.
    pub fn new(configs: &[GpuCacheConfig]) -> Self {
        let mut atlases = Vec::new();
        for config in configs {
            let size = config.texture_size.get();
            atlases.push(vec![0; size * size]);
        }

        Self {
            gpu_renderer: GpuRenderer::new(configs),
            atlases: std::cell::RefCell::new(atlases),
            atlas_configs: configs.to_vec(),
        }
    }

    /// Renders the layout into an RGBA target buffer.
    ///
    /// The `target_buffer` must be `target_width * target_height * 4` bytes.
    /// Color blending uses premultiplied alpha compositing.
    pub fn render<T: Clone + Copy + Into<[f32; 4]>>(
        &mut self,
        layout: &TextLayout<T>,
        font_storage: &mut FontStorage,
        target_buffer: &mut [u8],
        target_width: usize,
        target_height: usize,
    ) {
        let target_cell = std::cell::RefCell::new(target_buffer);

        self.gpu_renderer.render(
            layout,
            font_storage,
            &mut |updates: &[AtlasUpdate]| {
                let mut atlases = self.atlases.borrow_mut();
                for update in updates {
                    let atlas = &mut atlases[update.texture_index];
                    let atlas_width = self.atlas_configs[update.texture_index].texture_size.get();

                    for row in 0..update.height {
                        let src_start = row * update.width;
                        let src_end = src_start + update.width;
                        let dst_start = (update.y + row) * atlas_width + update.x;
                        let dst_end = dst_start + update.width;

                        if dst_end <= atlas.len() && src_end <= update.pixels.len() {
                            atlas[dst_start..dst_end]
                                .copy_from_slice(&update.pixels[src_start..src_end]);
                        }
                    }
                }
            },
            &mut |instances: &[GlyphInstance<T>]| {
                let mut target_buffer = target_cell.borrow_mut();
                let atlases = self.atlases.borrow();
                for instance in instances {
                    let color: [f32; 4] = instance.user_data.into();
                    let atlas = &atlases[instance.texture_index];
                    let atlas_width = self.atlas_configs[instance.texture_index]
                        .texture_size
                        .get();
                    let atlas_height = atlas_width; // Assuming square

                    // UV rect to pixel coordinates
                    let u_min = instance.uv_rect.min.x * atlas_width as f32;
                    let v_min = instance.uv_rect.min.y * atlas_height as f32;
                    let u_max = instance.uv_rect.max.x * atlas_width as f32;
                    let v_max = instance.uv_rect.max.y * atlas_height as f32;

                    let src_x = u_min.round() as usize;
                    let src_y = v_min.round() as usize;
                    let src_w = (u_max - u_min).round() as usize;
                    let src_h = (v_max - v_min).round() as usize;

                    let dst_x = instance.screen_rect.min.x.round() as i32;
                    let dst_y = instance.screen_rect.min.y.round() as i32;

                    // Simple blending
                    for dy in 0..src_h {
                        for dx in 0..src_w {
                            let sx = src_x + dx;
                            let sy = src_y + dy;

                            if sx >= atlas_width || sy >= atlas_height {
                                continue;
                            }

                            let alpha = atlas[sy * atlas_width + sx] as f32 / 255.0;
                            if alpha == 0.0 {
                                continue;
                            }

                            let tx = dst_x + dx as i32;
                            let ty = dst_y + dy as i32;

                            if tx < 0
                                || tx >= target_width as i32
                                || ty < 0
                                || ty >= target_height as i32
                            {
                                continue;
                            }

                            let pixel_idx = (ty as usize * target_width + tx as usize) * 4;

                            // Alpha blending
                            // Input color is premultiplied alpha
                            let src_r = color[0] * alpha;
                            let src_g = color[1] * alpha;
                            let src_b = color[2] * alpha;
                            let src_a = color[3] * alpha;

                            let bg_r = target_buffer[pixel_idx] as f32 / 255.0;
                            let bg_g = target_buffer[pixel_idx + 1] as f32 / 255.0;
                            let bg_b = target_buffer[pixel_idx + 2] as f32 / 255.0;
                            let bg_a = target_buffer[pixel_idx + 3] as f32 / 255.0;

                            let out_a = src_a + bg_a * (1.0 - src_a);
                            // Avoid division by zero
                            if out_a > 0.0 {
                                let out_r = (src_r + bg_r * bg_a * (1.0 - src_a)) / out_a;
                                let out_g = (src_g + bg_g * bg_a * (1.0 - src_a)) / out_a;
                                let out_b = (src_b + bg_b * bg_a * (1.0 - src_a)) / out_a;

                                target_buffer[pixel_idx] = (out_r * 255.0) as u8;
                                target_buffer[pixel_idx + 1] = (out_g * 255.0) as u8;
                                target_buffer[pixel_idx + 2] = (out_b * 255.0) as u8;
                                target_buffer[pixel_idx + 3] = (out_a * 255.0) as u8;
                            }
                        }
                    }
                }
            },
            &mut |standalone: &StandaloneGlyph<T>| {
                let mut target_buffer = target_cell.borrow_mut();
                let color: [f32; 4] = standalone.user_data.into();
                let src_w = standalone.width;
                let src_h = standalone.height;

                let dst_x = standalone.screen_rect.min.x.round() as i32;
                let dst_y = standalone.screen_rect.min.y.round() as i32;

                for dy in 0..src_h {
                    for dx in 0..src_w {
                        let alpha = standalone.pixels[dy * src_w + dx] as f32 / 255.0;
                        if alpha == 0.0 {
                            continue;
                        }

                        let tx = dst_x + dx as i32;
                        let ty = dst_y + dy as i32;

                        if tx < 0
                            || tx >= target_width as i32
                            || ty < 0
                            || ty >= target_height as i32
                        {
                            continue;
                        }

                        let pixel_idx = (ty as usize * target_width + tx as usize) * 4;

                        // Alpha blending
                        // Input color is premultiplied alpha
                        let src_r = color[0] * alpha;
                        let src_g = color[1] * alpha;
                        let src_b = color[2] * alpha;
                        let src_a = color[3] * alpha;

                        let bg_r = target_buffer[pixel_idx] as f32 / 255.0;
                        let bg_g = target_buffer[pixel_idx + 1] as f32 / 255.0;
                        let bg_b = target_buffer[pixel_idx + 2] as f32 / 255.0;
                        let bg_a = target_buffer[pixel_idx + 3] as f32 / 255.0;

                        let out_a = src_a + bg_a * (1.0 - src_a);
                        if out_a > 0.0 {
                            let out_r = (src_r + bg_r * bg_a * (1.0 - src_a)) / out_a;
                            let out_g = (src_g + bg_g * bg_a * (1.0 - src_a)) / out_a;
                            let out_b = (src_b + bg_b * bg_a * (1.0 - src_a)) / out_a;

                            target_buffer[pixel_idx] = (out_r * 255.0) as u8;
                            target_buffer[pixel_idx + 1] = (out_g * 255.0) as u8;
                            target_buffer[pixel_idx + 2] = (out_b * 255.0) as u8;
                            target_buffer[pixel_idx + 3] = (out_a * 255.0) as u8;
                        }
                    }
                }
            },
        );
    }
}
