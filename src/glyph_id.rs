pub const SUB_PIXEL_QUANTIZE: f32 = 256f32;

/// The same glyph is not guaranteed to receive the same `GlyphId` across program runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphId {
    font_id: fontdb::ID,
    glyph_index: u16,
    font_size: u32, // font size * SUB_PIXEL_QUANTIZE as u32
}

impl GlyphId {
    pub fn new(font_id: fontdb::ID, glyph_index: u16, font_size: f32) -> Self {
        Self {
            font_id,
            glyph_index,
            font_size: (font_size * SUB_PIXEL_QUANTIZE).round() as u32,
        }
    }

    pub fn font_id(&self) -> fontdb::ID {
        self.font_id
    }

    pub fn glyph_index(&self) -> u16 {
        self.glyph_index
    }

    pub fn font_size(&self) -> f32 {
        self.font_size as f32 / SUB_PIXEL_QUANTIZE
    }
}
