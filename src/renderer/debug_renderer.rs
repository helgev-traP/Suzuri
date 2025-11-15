use crate::font_storage::FontStorage;
use crate::text::{GlyphPosition, TextLayout};

/// Simple grayscale bitmap used for debugging text layout.
///
/// Pixels are stored in row-major order with the origin at the top-left.
/// Each pixel is a single byte where `0` is background and `255` is white.
pub struct DebugBitmap {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl DebugBitmap {
    fn new(width: u32, height: u32) -> Self {
        let len = width.saturating_mul(height) as usize;
        Self {
            width,
            height,
            pixels: vec![0; len],
        }
    }
}

/// Renders an existing `TextLayout` into a grayscale bitmap.
///
/// The caller is responsible for choosing the bitmap dimensions. This keeps the
/// renderer focused on glyph rasterization and clipping.
pub fn render_layout_to_bitmap(
    layout: &TextLayout,
    image_size: [usize; 2],
    font_storage: &mut FontStorage,
) -> DebugBitmap {
    let width = image_size[0] as u32;
    let height = image_size[1] as u32;

    // Empty layouts produce an empty bitmap so callers can distinguish the case.
    if width == 0 || height == 0 {
        return DebugBitmap::new(0, 0);
    }

    let mut bitmap = DebugBitmap::new(width, height);

    for line in &layout.lines {
        for glyph in &line.glyphs {
            render_glyph_into_bitmap(&mut bitmap, glyph, font_storage);
        }
    }

    bitmap
}

/// Renders a single glyph into the target bitmap.
///
/// The glyph is rasterized using `fontdue` at the size encoded in the
/// `GlyphId`. Coverage values are added to the existing pixel contents and
/// clamped to 255 to keep the bitmap valid.
fn render_glyph_into_bitmap(
    bitmap: &mut DebugBitmap,
    glyph_pos: &GlyphPosition,
    font_storage: &mut FontStorage,
) {
    let glyph_id = glyph_pos.glyph_id;
    let font_id = glyph_id.font_id();
    let glyph_index = glyph_id.glyph_index();
    let font_size = glyph_id.font_size();

    let Some(font) = font_storage.font(font_id) else {
        return;
    };

    let (metrics, coverage) = font.rasterize_indexed(glyph_index, font_size);

    if metrics.width == 0 || metrics.height == 0 {
        return;
    }

    let glyph_width = metrics.width as u32;
    let glyph_height = metrics.height as u32;

    let origin_x = glyph_pos.x;
    let origin_y = glyph_pos.y;

    for row in 0..glyph_height {
        for col in 0..glyph_width {
            let src_alpha = coverage[(row * glyph_width + col) as usize];
            if src_alpha == 0 {
                continue;
            }

            let x = origin_x + col as f32;
            let y = origin_y + row as f32;

            if x < 0.0 || y < 0.0 {
                continue;
            }

            let ix = x.floor() as u32;
            let iy = y.floor() as u32;

            if ix >= bitmap.width || iy >= bitmap.height {
                continue;
            }

            let idx = (iy * bitmap.width + ix) as usize;
            let existing = bitmap.pixels[idx] as u16;
            let added = src_alpha as u16;
            let combined = existing.saturating_add(added).min(255);
            bitmap.pixels[idx] = combined as u8;
        }
    }
}
