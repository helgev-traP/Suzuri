/// Quantization factor for font sizes to improve cache hit rates.
///
/// Font sizes are multiplied by this value and rounded to integers for cache lookups.
/// This allows small floating-point differences in font sizes to share cached glyphs.
pub const SUB_PIXEL_QUANTIZE: f32 = 256f32;

/// The same glyph is not guaranteed to receive the same `GlyphId` across program runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlyphId {
    font_id: fontdb::ID,
    glyph_index: u16,
    font_size: u32, // font size * SUB_PIXEL_QUANTIZE as u32
}

impl GlyphId {
    /// Creates a new `GlyphId` combining font, glyph, and size.
    ///
    /// The font size is quantized to allow better caching overlap for small size differences.
    pub fn new(font_id: fontdb::ID, glyph_index: u16, font_size: f32) -> Self {
        Self {
            font_id,
            glyph_index,
            font_size: (font_size * SUB_PIXEL_QUANTIZE).round() as u32,
        }
    }

    /// Returns the font ID.
    pub fn font_id(&self) -> fontdb::ID {
        self.font_id
    }

    /// Returns the glyph index.
    pub fn glyph_index(&self) -> u16 {
        self.glyph_index
    }

    /// Returns the font size.
    pub fn font_size(&self) -> f32 {
        self.font_size as f32 / SUB_PIXEL_QUANTIZE
    }
}
