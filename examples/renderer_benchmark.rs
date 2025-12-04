use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::time::Instant;

use fxhash::FxBuildHasher;
use wgfont::{
    font_storage::FontStorage,
    fontdb::{self, Family, Query},
    renderer::{CpuRenderer, cpu_renderer::GlyphCache, debug_renderer},
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
        let max_width = Some(800.0);
        let max_height = None;
        make_config(max_width, max_height)
    };

    let mut font_storage = FontStorage::new();
    let font_id = pick_system_font(&mut font_storage);

    // Create a reasonably long text to make the rendering work significant
    let mut data = TextData::new();
    let text_content = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
        Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. \
        Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. \
        Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. \
        Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.\n\n".repeat(5);

    data.append(TextElement {
        font_id,
        font_size: 24.0,
        content: text_content,
        user_data: (),
    });

    // Perform layout once
    println!("Performing layout...");
    let layout = data.layout(&config, &mut font_storage);
    println!(
        "Layout ready: {} lines, size: {}x{}",
        layout.lines.len(),
        layout.total_width,
        layout.total_height
    );

    let bitmap_width = config.max_width.unwrap_or(layout.total_width).ceil() as usize;
    let bitmap_height = config.max_height.unwrap_or(layout.total_height).ceil() as usize;
    let image_size = [bitmap_width, bitmap_height];

    let iterations = 100;
    println!("\nStarting benchmark ({} iterations)...", iterations);

    // --- Benchmark Debug Renderer ---
    {
        let start = Instant::now();
        for _ in 0..iterations {
            let bitmap =
                debug_renderer::render_layout_to_bitmap(&layout, image_size, &mut font_storage);
            // Prevent optimization
            std::hint::black_box(bitmap);
        }
        let duration = start.elapsed();
        println!(
            "Debug Renderer: Total: {:.2?}, Avg: {:.2?}",
            duration,
            duration / iterations
        );
    }

    // --- Benchmark CPU Renderer ---
    {
        // Configure cache
        let cache_config = [
            (
                NonZeroUsize::new(512).unwrap(), // Block size
                NonZeroUsize::new(128).unwrap(), // Capacity
            ),
            (
                NonZeroUsize::new(1024).unwrap(),
                NonZeroUsize::new(128).unwrap(),
            ),
        ];
        let cache = GlyphCache::new(&cache_config);
        let mut renderer = CpuRenderer::new(cache);

        // Warmup / First run (includes caching overhead)
        let start_first = Instant::now();
        let mut bitmap = debug_renderer::Bitmap::new(image_size[0], image_size[1]);
        renderer.render(
            &layout,
            image_size,
            &mut font_storage,
            &mut |pos, alpha, _| bitmap.accumulate(pos[0], pos[1], alpha),
        );
        std::hint::black_box(bitmap);
        let duration_first = start_first.elapsed();
        println!("Cpu Renderer (First Run): {:.2?}", duration_first);

        // Cached runs
        let start = Instant::now();
        for _ in 0..iterations {
            let mut bitmap = debug_renderer::Bitmap::new(image_size[0], image_size[1]);
            renderer.render(
                &layout,
                image_size,
                &mut font_storage,
                &mut |pos, alpha, _| bitmap.accumulate(pos[0], pos[1], alpha),
            );
            std::hint::black_box(bitmap);
        }
        let duration = start.elapsed();
        println!(
            "Cpu Renderer (Cached): Total: {:.2?}, Avg: {:.2?}",
            duration,
            duration / iterations
        );
    }
}
