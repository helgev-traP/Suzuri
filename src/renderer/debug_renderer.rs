use crate::font_storage::FontStorage;
use crate::renderer::Bitmap;
use crate::text::{GlyphPosition, TextLayout};

/// Renders an existing `TextLayout` into a grayscale bitmap.
///
/// The caller is responsible for choosing the bitmap dimensions. This keeps the
/// renderer focused on glyph rasterization and clipping.
pub fn render_layout_to_bitmap(
    layout: &TextLayout,
    image_size: [usize; 2],
    font_storage: &mut FontStorage,
) -> Bitmap {
    let width = image_size[0];
    let height = image_size[1];

    // Empty layouts produce an empty bitmap so callers can distinguish the case.
    if width == 0 || height == 0 {
        return Bitmap::new(0, 0);
    }

    let mut bitmap = Bitmap::new(width, height);

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
    bitmap: &mut Bitmap,
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
        let y = origin_y + row as f32;
        // Skip rows strictly above the canvas
        if y < 0.0 {
            continue;
        }

        let iy = y.floor() as isize;
        // Skip rows strictly below the canvas (or above if floor pushed it negative, though y < 0 check handles most)
        if iy < 0 || iy as usize >= bitmap.height {
            continue;
        }

        for col in 0..glyph_width {
            let src_alpha = coverage[(row * glyph_width + col) as usize];
            if src_alpha == 0 {
                continue;
            }

            let x = origin_x + col as f32;
            if x < 0.0 {
                continue;
            }

            let ix = x.floor() as isize;
            if ix < 0 {
                continue;
            }

            // The accumulate method handles the width/height bounds check safely
            bitmap.accumulate(ix as usize, iy as usize, src_alpha);
        }
    }
}
