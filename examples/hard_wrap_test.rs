use std::num::NonZeroUsize;

use image::{ImageBuffer, Rgb};
use suzuri::{
    FontSystem,
    renderer::CpuCacheConfig,
    text::{TextData, TextElement},
};

mod example_common;
use example_common::{TextColor, load_fonts, make_layout_config};

const TEST_WIDTH: f32 = 100.0;

#[allow(clippy::unwrap_used)]
fn main() {
    // 1. Setup Layout Config with a small width to force hard wrap
    let mut config = make_layout_config(Some(TEST_WIDTH), None);
    config.wrap_hard_break = true;

    // 2. Setup Font System
    let font_system = FontSystem::new();
    let (heading_font, body_font, _mono_font) = load_fonts(&font_system);

    // 3. Create TextData with a very long word
    let mut data = TextData::new();
    data.append(TextElement {
        font_id: heading_font,
        font_size: 24.0,
        content: "HardWalk:\n".into(),
        user_data: TextColor::NEON_PINK,
    });
    data.append(TextElement {
        font_id: body_font,
        font_size: 18.0,
        // formatted as a single long word without spaces
        content:
            "SuperCalifoRagiListicExpoaliDociousEvenThoughTheSoundOfItIsSomethingQuiteAtrocious\n"
                .into(),
        user_data: TextColor::WHITE,
    });
    data.append(TextElement {
        font_id: body_font,
        font_size: 14.0,
        content: "\n(The word above should be broken across multiple lines)".into(),
        user_data: TextColor::MUTED_GRAY,
    });

    // 4. Perform Layout
    let layout = font_system.layout_text(&data, &config);

    println!("Layout Area: {:.2}x{:.2}", TEST_WIDTH, layout.total_height);
    println!(
        "Result Size: {:.2}x{:.2} lines={}",
        layout.total_width,
        layout.total_height,
        layout.lines.len()
    );

    // 5. Initialize CPU Renderer (no-default-features friendly)
    let bitmap_width = TEST_WIDTH.ceil() as usize;
    let bitmap_height = layout.total_height.ceil() as usize;

    let cache_config = [CpuCacheConfig {
        block_size: NonZeroUsize::new(256).unwrap(),
        capacity: NonZeroUsize::new(128).unwrap(),
    }];
    font_system.cpu_init(&cache_config);

    // 6. Render to Image
    let mut image_buffer: ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(bitmap_width as u32, bitmap_height as u32, Rgb([20, 20, 25]));

    font_system.cpu_render(
        &layout,
        [bitmap_width, bitmap_height],
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

    // 7. Save Output
    std::fs::create_dir_all("debug").expect("failed to create debug directory");
    let output_path = "debug/hard_wrap_test.png";
    image_buffer
        .save(output_path)
        .expect("failed to save image");

    println!("Saved debug image to: {}", output_path);
}
