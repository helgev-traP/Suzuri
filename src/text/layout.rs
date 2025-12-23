use std::collections::HashSet;
use crate::{glyph_id::GlyphId, text::TextData};

/// Configuration knobs used by the text layout pipeline.
///
/// All parameters are honored during a single `TextData::layout` call so the
/// caller can measure or place text inside arbitrary rectangles.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayoutConfig {
    /// Maximum width of the layout box. If text exceeds this, it may wrap or overflow.
    pub max_width: Option<f32>,
    /// Maximum height of the layout box.
    pub max_height: Option<f32>,
    /// Horizontal alignment of the text within the layout box.
    pub horizontal_align: HorizontalAlign,
    /// Vertical alignment of the text within the layout box.
    pub vertical_align: VerticalAlign,
    /// Scaling factor for the line height.
    pub line_height_scale: f32,
    /// Strategy for wrapping text.
    pub wrap_style: WrapStyle,
    /// Whether to force a hard break when text exceeds width, even in the middle of a word (if word wrapping fails).
    pub wrap_hard_break: bool,
}

impl Default for TextLayoutConfig {
    fn default() -> Self {
        Self {
            max_width: None,
            max_height: None,
            horizontal_align: HorizontalAlign::Left,
            vertical_align: VerticalAlign::Top,
            line_height_scale: 1.0,
            wrap_style: WrapStyle::NoWrap,
            wrap_hard_break: false,
        }
    }
}

/// Horizontal justification applied after each line is assembled.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HorizontalAlign {
    /// Align text to the left.
    #[default]
    Left,
    /// Center text horizontally.
    Center,
    /// Align text to the right.
    Right,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Vertical alignment strategy for the entire block of text.
pub enum VerticalAlign {
    /// Align text to the top.
    #[default]
    Top,
    /// Center text vertically.
    Middle,
    /// Align text to the bottom.
    Bottom,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Wrapping rules that define where line breaks may occur.
pub enum WrapStyle {
    /// Wrap text at word boundaries.
    #[default]
    WordWrap,
    /// Wrap text at any character.
    CharWrap,
    /// Do not wrap text.
    NoWrap,
}

/// Final layout output produced by [`TextData::layout`].
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayout<T> {
    /// The configuration used for this layout.
    pub config: TextLayoutConfig,
    /// The total height of the laid out text.
    pub total_height: f32,
    /// The total width of the laid out text.
    pub total_width: f32,
    /// The lines of text in the layout.
    pub lines: Vec<TextLayoutLine<T>>,
}

impl<T> TextLayout<T> {
    /// Returns the number of lines in the layout.
    pub fn len_lines(&self) -> usize {
        self.lines.len()
    }

    /// Returns the total number of glyphs in the layout (sum of glyphs in all lines).
    pub fn len_glyphs(&self) -> usize {
        self.lines.iter().map(|line| line.glyphs.len()).sum()
    }
}

/// A single row of positioned glyphs in the final layout.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayoutLine<T> {
    /// The height of this line.
    pub line_height: f32,
    /// The width of this line.
    pub line_width: f32,
    /// The Y coordinate of the top of this line.
    pub top: f32,
    /// The Y coordinate of the bottom of this line.
    pub bottom: f32,
    /// The glyphs contained in this line.
    pub glyphs: Vec<GlyphPosition<T>>,
}

/// **Y-axis goes down**
///
/// Each glyph uses the global coordinates generated during layout so renderers
/// can draw them directly without additional transformations.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphPosition<T> {
    /// The unique identifier for the glyph.
    pub glyph_id: GlyphId,
    /// The absolute X coordinate of the glyph.
    pub x: f32,
    /// The absolute Y coordinate of the glyph.
    pub y: f32,
    /// Custom user data associated with this glyph.
    pub user_data: T,
}
// place holder for eq and hash
// todo: consider another way
impl<T: Eq> Eq for GlyphPosition<T> {}
impl<T: std::hash::Hash> std::hash::Hash for GlyphPosition<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.glyph_id.hash(state);
        self.x.to_bits().hash(state);
        self.y.to_bits().hash(state);
        self.user_data.hash(state);
    }
}

fn is_space(s: &str) -> (bool, &str) {
    todo!()
}

fn is_linebreak(s: &str) -> (bool, &str) {
    todo!()
}

impl<T: Clone> TextData<T> {
    /// Computes the bounding box that would be produced by [`Self::layout`].
    ///
    /// This helper simply forwards to `layout` because the layout stage must
    /// still run to honor wrapping, alignment, and kerning rules. The resulting
    /// size is returned as `[width, height]` for convenience.
    pub fn measure(
        &self,
        config: &TextLayoutConfig,
        font_storage: &mut crate::font_storage::FontStorage,
    ) -> [f32; 2] {
        todo!()
    }

    /// Performs glyph layout according to the provided configuration.
    ///
    /// The implementation follows a two-stage pipeline:
    /// 1. Each input character is translated into glyph fragments that are
    ///    buffered into line records while respecting wrap style and width
    ///    constraints.
    /// 2. The buffered lines are converted into final glyph positions with
    ///    alignment offsets applied.
    ///
    /// Breaking the work into stages keeps the code readable and allows future
    /// extensions such as hyphenation without rewriting the core placement
    /// logic.
    pub fn layout(
        &self,
        config: &TextLayoutConfig,
        font_storage: &mut crate::font_storage::FontStorage,
    ) -> TextLayout<T> {
        todo!()
    }
}
