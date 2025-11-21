use std::collections::HashSet;
use std::num::NonZeroUsize;

use fxhash::FxBuildHasher;
use image::{ImageBuffer, Luma};
use wgfont::{
    font_storage::FontStorage,
    fontdb::{self, Family, Query},
    renderer::{CpuRenderer, cpu_renderer::GlyphCache},
    text::{HorizontalAlign, TextData, TextElement, TextLayoutConfig, VerticalAlign, WrapStyle},
};

fn make_config(max_width: Option<f32>, max_height: Option<f32>) -> TextLayoutConfig {
    let mut word_separators: HashSet<char, FxBuildHasher> =
        HashSet::with_hasher(FxBuildHasher::default());
    word_separators.insert(' ');
    word_separators.insert('\t');
    word_separators.insert(',');
    word_separators.insert('.');

    let mut linebreak_char: HashSet<char, FxBuildHasher> =
        HashSet::with_hasher(FxBuildHasher::default());
    linebreak_char.insert('\n');

    TextLayoutConfig {
        max_width,
        max_height,
        horizontal_align: HorizontalAlign::Left,
        vertical_align: VerticalAlign::Top,
        line_height_scale: 1.0,
        wrap_style: WrapStyle::WordWrap,
        wrap_hard_break: true,
        word_separators,
        linebreak_char,
    }
}

fn pick_system_font(font_storage: &mut FontStorage) -> fontdb::ID {
    font_storage.load_system_fonts();
    assert!(
        !font_storage.is_empty(),
        "system fonts are required for the text layout test"
    );

    const FAMILIES: &[Family<'_>] = &[Family::SansSerif];
    let query = Query {
        families: FAMILIES,
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };

    if let Some((font_id, _)) = font_storage.query(&query) {
        return font_id;
    }

    font_storage
        .faces()
        .next()
        .map(|face| face.id)
        .expect("no usable fonts registered in FontStorage")
}

#[allow(clippy::unwrap_used)]
fn main() {
    let config = {
        let max_width = Some(400.0);
        let max_height = None;
        make_config(max_width, max_height)
    };

    let mut font_storage = FontStorage::new();
    let font_id = pick_system_font(&mut font_storage);

    let mut data = TextData::new();
    data.append(TextElement {
        font_id,
        font_size: 32.0,
        content: "CpuRenderer text rendering example.\n\
                  This text is rendered using the CPU renderer with a glyph cache.\n\
                  It supports caching and reuse of rasterized glyphs."
            .into(),
    });

    // Layout
    let layout = data.layout(&config, &mut font_storage);

    println!(
        "Layout: total_width={} total_height={} lines={}",
        layout.total_width,
        layout.total_height,
        layout.lines.len()
    );

    let bitmap_width = config.max_width.unwrap_or(layout.total_width).ceil() as usize;
    let bitmap_height = config.max_height.unwrap_or(layout.total_height).ceil() as usize;

    // Initialize CpuRenderer
    let cache_config = [
        (
            NonZeroUsize::new(1024).unwrap(), // Block size (e.g. 32x32)
            NonZeroUsize::new(1024).unwrap(), // Capacity
        ),
        (
            NonZeroUsize::new(4096).unwrap(), // Block size (e.g. 64x64)
            NonZeroUsize::new(256).unwrap(),  // Capacity
        ),
    ];
    let cache = GlyphCache::new(&cache_config);
    let mut renderer = CpuRenderer::new(cache);

    // Render
    let timer = std::time::Instant::now();
    let bitmap = renderer.render_layout(&layout, [bitmap_width, bitmap_height], &mut font_storage);
    let elapsed = timer.elapsed();

    println!(
        "Rendered image: width={} height={} (elapsed: {:.2?})",
        bitmap.width, bitmap.height, elapsed
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

    img_buffer
        .save("debug/cpu_renderer_text.png")
        .expect("failed to save debug image");

    println!("Saved debug image to debug/cpu_renderer_text.png");
}
