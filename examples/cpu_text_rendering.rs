use std::num::NonZeroUsize;

use image::{ImageBuffer, Rgb};
use wgfont::{font_storage::FontStorage, renderer::CpuRenderer};

mod example_common;
use example_common::{TextColor, WIDTH, build_text_data, load_fonts, make_layout_config};

#[allow(clippy::unwrap_used)]
fn main() {
    let config = make_layout_config(Some(WIDTH), None);

    let mut font_storage = FontStorage::new();
    let (heading_font, body_font, mono_font) = load_fonts(&mut font_storage);
    let data = build_text_data(heading_font, body_font, mono_font);

    // Layout
    let layout_timer = std::time::Instant::now();
    let layout = data.layout(&config, &mut font_storage);
    let layout_elapsed = layout_timer.elapsed();

    println!(
        "Layout: {:.2}x{:.2} lines={} (elapsed: {:.2?})",
        layout.total_width,
        layout.total_height,
        layout.lines.len(),
        layout_elapsed
    );

    let bitmap_width = WIDTH.ceil() as usize;
    let bitmap_height = layout.total_height.ceil() as usize;

    // Initialize CpuRenderer
    let cache_config = [
        wgfont::renderer::cpu_renderer::CpuCacheConfig {
            block_size: NonZeroUsize::new(1024).unwrap(), // Block size (e.g. 32x32)
            capacity: NonZeroUsize::new(1024).unwrap(),   // Capacity
        },
        wgfont::renderer::cpu_renderer::CpuCacheConfig {
            block_size: NonZeroUsize::new(4096).unwrap(), // Block size (e.g. 64x64)
            capacity: NonZeroUsize::new(256).unwrap(),    // Capacity
        },
    ];
    let mut renderer = CpuRenderer::new(&cache_config);

    // Render
    // Note: CPU renderer is Grayscale-only (coverage), so we'll render to a colored image manually
    // by blending the text color with the coverage.
    let mut image_buffer: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(bitmap_width as u32, bitmap_height as u32, Rgb([20, 20, 25])); // Dark background

    let mut measurements = Vec::new();
    for i in 0..2 {
        let timer = std::time::Instant::now();
        // Reset buffer for the first pass (optional, but cleaner) or just draw over.
        // We just draw over to avoid re-allocation or heavy clear costs affecting the benchmark if included (though we measure inside loop).
        // Actually, let's just draw.
        // Note: The second pass will blend onto the first pass result, making it brighter/messier, but timing is what matters.

        renderer.render(
            &layout,
            [bitmap_width, bitmap_height],
            &mut font_storage,
            &mut |pos, alpha, color: &TextColor| {
                let alpha_f = alpha as f32 / 255.0;
                if alpha_f <= 0.0 {
                    return;
                }
                let x = pos[0] as u32;
                let y = pos[1] as u32;
                if x >= bitmap_width as u32 || y >= bitmap_height as u32 {
                    return;
                }

                let pixel = image_buffer.get_pixel_mut(x, y);
                let bg_r = pixel[0] as f32 / 255.0;
                let bg_g = pixel[1] as f32 / 255.0;
                let bg_b = pixel[2] as f32 / 255.0;

                // Simple alpha blending
                let out_r = color.r * alpha_f + bg_r * (1.0 - alpha_f);
                let out_g = color.g * alpha_f + bg_g * (1.0 - alpha_f);
                let out_b = color.b * alpha_f + bg_b * (1.0 - alpha_f);

                *pixel = Rgb([
                    (out_r.clamp(0.0, 1.0) * 255.0) as u8,
                    (out_g.clamp(0.0, 1.0) * 255.0) as u8,
                    (out_b.clamp(0.0, 1.0) * 255.0) as u8,
                ]);
            },
        );
        measurements.push(timer.elapsed());
        if i == 0 {
            // For the sake of the output image quality, we might want to clear it if we were saving the result of the second pass.
            // But we are saving whatever is in image_buffer at the end.
            // If we don't clear, the image is double-drawn.
            // For debugging purposes, let's clear it so the saved image is correct (from the 2nd pass).
            // We won't include this clear in the timing.
            image_buffer = ImageBuffer::from_pixel(
                bitmap_width as u32,
                bitmap_height as u32,
                Rgb([20, 20, 25]),
            );
        }
    }

    println!(
        "Render (1st): {}x{} (elapsed: {:.2?})",
        bitmap_width, bitmap_height, measurements[0]
    );
    println!(
        "Render (2nd): {}x{} (elapsed: {:.2?})",
        bitmap_width, bitmap_height, measurements[1]
    );

    // Ensure debug directory exists
    std::fs::create_dir_all("debug").expect("failed to create debug directory");

    let output_path = "debug/cpu_renderer_text.png";
    image_buffer
        .save(output_path)
        .expect("failed to save debug image");

    println!("Saved: {}", output_path);
}
