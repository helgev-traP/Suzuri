use std::collections::HashSet;
use std::num::NonZeroUsize;

use fxhash::FxBuildHasher;
use image::{ImageBuffer, Rgba};
use wgfont::{
    font_storage::FontStorage,
    fontdb::{self, Family, Query},
    renderer::{
        cpu_debug_renderer::CpuDebugRenderer, gpu_renderer::GlyphAtlasConfig,
        wgpu_renderer::ToInstance,
    },
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

// Define a color type for the example
#[derive(Clone, Copy, Debug)]
struct TextColor {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl ToInstance for TextColor {
    fn to_color(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

fn main() {
    // 1. Setup CpuDebugRenderer
    let configs = vec![
        GlyphAtlasConfig {
            tile_size: NonZeroUsize::new(32).unwrap(),
            tiles_per_axis: NonZeroUsize::new(16).unwrap(),
            texture_size: NonZeroUsize::new(512).unwrap(),
        },
        GlyphAtlasConfig {
            tile_size: NonZeroUsize::new(64).unwrap(),
            tiles_per_axis: NonZeroUsize::new(8).unwrap(),
            texture_size: NonZeroUsize::new(512).unwrap(),
        },
    ];

    let mut renderer = CpuDebugRenderer::new(configs);

    // 2. Setup Text Layout
    let config = {
        let max_width = Some(800.0);
        let max_height = None;
        make_config(max_width, max_height)
    };

    let mut font_storage = FontStorage::new();
    let font_id = pick_system_font(&mut font_storage);

    let mut data = TextData::new();
    // 1. Title
    data.append(TextElement {
        font_id,
        font_size: 48.0, // Unique size
        content: "Typography Showcase\n".into(),
        user_data: TextColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }, // White
    });
    data.append(TextElement {
        font_id,
        font_size: 24.1, // Unique size
        content: "\n".into(),
        user_data: TextColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    });

    // 2. Introduction (Regular text)
    data.append(TextElement {
        font_id,
        font_size: 18.1, // Unique size
        content: "This example demonstrates the capabilities of the ".into(),
        user_data: TextColor {
            r: 0.8,
            g: 0.8,
            b: 0.8,
            a: 1.0,
        }, // Light Gray
    });
    data.append(TextElement {
        font_id,
        font_size: 18.2, // Unique size
        content: "WgpuRenderer".into(),
        user_data: TextColor {
            r: 0.4,
            g: 0.8,
            b: 1.0,
            a: 1.0,
        }, // Light Blue
    });
    data.append(TextElement {
        font_id,
        font_size: 18.3, // Unique size
        content: ". It supports efficient batching, dynamic atlas management, and standalone rendering for large glyphs.\n\n".into(),
        user_data: TextColor { r: 0.8, g: 0.8, b: 0.8, a: 1.0 },
    });

    // 3. Features List (Mixed styles)
    data.append(TextElement {
        font_id,
        font_size: 24.2, // Unique size
        content: "Key Features:\n".into(),
        user_data: TextColor {
            r: 1.0,
            g: 0.8,
            b: 0.4,
            a: 1.0,
        }, // Gold
    });

    // Feature 1: Colors
    data.append(TextElement {
        font_id,
        font_size: 20.1, // Unique size
        content: "• Rich Colors: ".into(),
        user_data: TextColor {
            r: 0.9,
            g: 0.9,
            b: 0.9,
            a: 1.0,
        },
    });
    data.append(TextElement {
        font_id,
        font_size: 20.2, // Unique size
        content: "Red, ".into(),
        user_data: TextColor {
            r: 1.0,
            g: 0.4,
            b: 0.4,
            a: 1.0,
        },
    });
    data.append(TextElement {
        font_id,
        font_size: 20.3, // Unique size
        content: "Green, ".into(),
        user_data: TextColor {
            r: 0.4,
            g: 1.0,
            b: 0.4,
            a: 1.0,
        },
    });
    data.append(TextElement {
        font_id,
        font_size: 20.4, // Unique size
        content: "and Blue.\n".into(),
        user_data: TextColor {
            r: 0.4,
            g: 0.4,
            b: 1.0,
            a: 1.0,
        },
    });

    // Feature 2: Sizes
    data.append(TextElement {
        font_id,
        font_size: 20.5, // Unique size
        content: "• Varied Sizes: ".into(),
        user_data: TextColor {
            r: 0.9,
            g: 0.9,
            b: 0.9,
            a: 1.0,
        },
    });
    data.append(TextElement {
        font_id,
        font_size: 12.1, // Unique size
        content: "Tiny ".into(),
        user_data: TextColor {
            r: 0.7,
            g: 0.7,
            b: 0.7,
            a: 1.0,
        },
    });
    data.append(TextElement {
        font_id,
        font_size: 20.6, // Unique size
        content: "Normal ".into(),
        user_data: TextColor {
            r: 0.9,
            g: 0.9,
            b: 0.9,
            a: 1.0,
        },
    });
    data.append(TextElement {
        font_id,
        font_size: 36.1, // Unique size
        content: "Huge!\n".into(),
        user_data: TextColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    });

    // Feature 3: Large Glyphs (Standalone)
    data.append(TextElement {
        font_id,
        font_size: 20.7, // Unique size
        content: "• Large Glyphs (Standalone Path):\n".into(),
        user_data: TextColor {
            r: 0.9,
            g: 0.9,
            b: 0.9,
            a: 1.0,
        },
    });
    data.append(TextElement {
        font_id,
        font_size: 120.1, // Unique size
        content: "A".into(),
        user_data: TextColor {
            r: 1.0,
            g: 0.2,
            b: 0.8,
            a: 1.0,
        }, // Magenta
    });
    data.append(TextElement {
        font_id,
        font_size: 120.2, // Unique size
        content: "B".into(),
        user_data: TextColor {
            r: 0.2,
            g: 1.0,
            b: 0.8,
            a: 1.0,
        }, // Cyan
    });
    data.append(TextElement {
        font_id,
        font_size: 120.3, // Unique size
        content: "C".into(),
        user_data: TextColor {
            r: 0.8,
            g: 1.0,
            b: 0.2,
            a: 1.0,
        }, // Lime
    });
    data.append(TextElement {
        font_id,
        font_size: 20.8, // Unique size
        content: "\n".into(),
        user_data: TextColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    });

    // 4. Lorem Ipsum (Long text for wrapping)
    data.append(TextElement {
        font_id,
        font_size: 16.1, // Unique size
        content: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.".into(),
        user_data: TextColor { r: 0.6, g: 0.6, b: 0.7, a: 1.0 }, // Muted Blue-Gray
    });

    let layout = data.layout(&config, &mut font_storage);

    let width = 832;
    let height = 600;

    // 3. Render
    let mut buffer = vec![0; width * height * 4];
    renderer.render(&layout, &mut font_storage, &mut buffer, width, height);

    // 4. Save
    std::fs::create_dir_all("debug").expect("failed to create debug directory");

    let img_buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width as u32, height as u32, buffer)
            .expect("failed to create image buffer");

    img_buffer
        .save("debug/cpu_debug_renderer_text.png")
        .expect("failed to save image");

    println!("Saved rendered image to debug/cpu_debug_renderer_text.png");
}
