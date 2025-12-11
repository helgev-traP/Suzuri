use image::{ImageBuffer, Luma};
use suzuri::{font_storage::FontStorage, renderer::debug_renderer};

mod example_common;
use example_common::{WIDTH, build_text_data, load_fonts, make_layout_config};

#[allow(clippy::unwrap_used)]
fn main() {
    let config = make_layout_config(Some(WIDTH), None);

    let mut font_storage = FontStorage::new();
    let (heading_font, body_font, mono_font) = load_fonts(&mut font_storage);
    let data = build_text_data(heading_font, body_font, mono_font);

    let timer = std::time::Instant::now();
    let layout = data.layout(&config, &mut font_storage);
    let elapsed = timer.elapsed();
    println!(
        "Layout: {:.2}x{:.2} lines={} (elapsed: {:.2?})",
        layout.total_width,
        layout.total_height,
        layout.lines.len(),
        elapsed
    );

    let bitmap_width = WIDTH.ceil() as usize;
    let bitmap_height = layout.total_height.ceil() as usize;

    let mut measurements = Vec::new();
    let mut last_bitmap = None;

    for _ in 0..2 {
        let render_timer = std::time::Instant::now();
        let bitmap = debug_renderer::render_layout_to_bitmap(
            &layout,
            [bitmap_width, bitmap_height],
            &mut font_storage,
        );
        measurements.push(render_timer.elapsed());
        last_bitmap = Some(bitmap);
    }
    let bitmap = last_bitmap.unwrap();

    println!(
        "Render (1st): {}x{} (elapsed: {:.2?})",
        bitmap.width, bitmap.height, measurements[0]
    );
    println!(
        "Render (2nd): {}x{} (elapsed: {:.2?})",
        bitmap.width, bitmap.height, measurements[1]
    );

    if bitmap.width == 0 || bitmap.height == 0 {
        println!("Bitmap is empty; nothing to write.");
        return;
    }

    // Ensure debug directory exists
    std::fs::create_dir_all("debug").expect("failed to create debug directory");

    let img_buffer: ImageBuffer<Luma<u8>, Vec<u8>> =
        ImageBuffer::from_raw(bitmap.width as u32, bitmap.height as u32, bitmap.pixels)
            .expect("bitmap dimensions must match pixel buffer length");

    let output_path = "debug/debug_text.png";
    img_buffer
        .save(output_path)
        .expect("failed to save debug image");

    println!("Saved: {}", output_path);
}
