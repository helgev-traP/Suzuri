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
    /// Characters that are considered word separators for wrapping.
    pub word_separators: HashSet<char, fxhash::FxBuildHasher>,
    /// Characters that trigger a hard line break.
    pub linebreak_char: HashSet<char, fxhash::FxBuildHasher>,
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
            // TODO: implement tab handling.
            word_separators: [' ', '\t', '\n', '\r'].iter().cloned().collect(),
            linebreak_char: ['\n', '\r'].iter().cloned().collect(),
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

/// Intermediate storage used while collecting glyphs for a single line.
struct LineRecord<T> {
    buffer: Option<layout_utl::LayoutBuffer<T>>,
    metrics: Option<fontdue::LineMetrics>,
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
        let layout = self.layout(config, font_storage);
        [layout.total_width, layout.total_height]
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
        use std::sync::Arc;

        // Stage 1: build intermediate line buffers and decide where wrapping
        // should occur. Stage 2: convert each line buffer into positioned
        // glyphs with alignment offsets applied.
        let max_width = config.max_width;
        let max_height = config.max_height;
        let horizontal_align = config.horizontal_align;
        let vertical_align = config.vertical_align;
        let line_height_scale = config.line_height_scale;
        let wrap_style = config.wrap_style;
        let wrap_hard_break = config.wrap_hard_break;
        let word_separators = &config.word_separators;
        let linebreak_char = &config.linebreak_char;

        /// Final measurements for a single laid-out line before alignment.
        struct LineData<T> {
            width: f32,
            height: f32,
            y: f32,
            glyphs: Vec<GlyphPosition<T>>,
        }

        let mut lines: Vec<LineRecord<T>> = Vec::new();
        let mut line_buf: Option<layout_utl::LayoutBuffer<T>> = None;
        let mut word_buf: Option<Vec<layout_utl::GlyphFragment<T>>> = None;
        let mut last_line_metrics: Option<fontdue::LineMetrics> = None;

        for text in &self.texts {
            let Some(font) = font_storage.font(text.font_id) else {
                continue;
            };
            let Some(line_metric) = font.horizontal_line_metrics(text.font_size) else {
                continue;
            };
            if text.content.is_empty() {
                continue;
            }

            last_line_metrics = Some(line_metric);

            // Helper closure to create a glyph fragment.
            let create_fragment = |ch: char| {
                let glyph_idx = font.lookup_glyph_index(ch);
                let metrics = font.metrics_indexed(glyph_idx, text.font_size);
                layout_utl::GlyphFragment {
                    ch,
                    glyph_idx,
                    metrics,
                    line_metrics: line_metric,
                    font_id: text.font_id,
                    font_size: text.font_size,
                    font: Arc::clone(&font),
                    user_data: text.user_data.clone(),
                }
            };

            for ch in text.content.chars() {
                match layout_utl::classify_char(ch, word_separators, linebreak_char) {
                    layout_utl::CharBehavior::LineBreak => {
                        // Terminate the current line.
                        if let Some(word) = word_buf.take() {
                            Self::append_fragments_with_rules(
                                &mut line_buf,
                                &mut lines,
                                &word,
                                true,
                                max_width,
                                wrap_style,
                                wrap_hard_break,
                                font_storage,
                            );
                        }

                        // Explicitly do not append the newline glyph to prevent artifacts.
                        Self::finalize_line(&mut line_buf, &mut lines, Some(line_metric));
                    }
                    layout_utl::CharBehavior::WordBreak { render_glyph } => {
                        // Word break encountered.
                        if let Some(word) = word_buf.take() {
                            Self::append_fragments_with_rules(
                                &mut line_buf,
                                &mut lines,
                                &word,
                                true,
                                max_width,
                                wrap_style,
                                wrap_hard_break,
                                font_storage,
                            );
                        }

                        if render_glyph {
                            let fragment = create_fragment(ch);
                            // Append the separator itself (e.g., space).
                            Self::append_fragments_with_rules(
                                &mut line_buf,
                                &mut lines,
                                std::slice::from_ref(&fragment),
                                false,
                                max_width,
                                wrap_style,
                                wrap_hard_break,
                                font_storage,
                            );
                        }
                    }
                    layout_utl::CharBehavior::Regular => {
                        let fragment = create_fragment(ch);
                        if matches!(wrap_style, WrapStyle::CharWrap) {
                            Self::append_fragments_with_rules(
                                &mut line_buf,
                                &mut lines,
                                std::slice::from_ref(&fragment),
                                true,
                                max_width,
                                wrap_style,
                                wrap_hard_break,
                                font_storage,
                            );
                        } else {
                            match &mut word_buf {
                                Some(buffer) => buffer.push(fragment),
                                None => word_buf = Some(vec![fragment]),
                            }
                        }
                    }
                    layout_utl::CharBehavior::Ignore => {
                        // Skip control characters or other ignored chars.
                    }
                }
            }
        }

        if let Some(word) = word_buf.take() {
            Self::append_fragments_with_rules(
                &mut line_buf,
                &mut lines,
                &word,
                true,
                max_width,
                wrap_style,
                wrap_hard_break,
                font_storage,
            );
        }

        Self::finalize_line(&mut line_buf, &mut lines, last_line_metrics);

        let mut layout_lines: Vec<LineData<T>> = Vec::new();
        let mut cursor_y = 0.0;
        let mut max_line_width: f32 = 0.0;

        for mut record in lines {
            let (width, ascent, descent, line_gap, glyphs) =
                if let Some(buffer) = record.buffer.take() {
                    let (ascent, descent, line_gap) = buffer.line_metrics();
                    let width_value = buffer.width();
                    let glyphs = buffer.glyphs;
                    (width_value, ascent, descent, line_gap, glyphs)
                } else if let Some(metrics) = record.metrics {
                    (
                        0.0,
                        metrics.ascent,
                        metrics.descent,
                        metrics.line_gap,
                        Vec::new(),
                    )
                } else {
                    (0.0, 0.0, 0.0, 0.0, Vec::new())
                };

            max_line_width = max_line_width.max(width);
            let raw_line_height = ascent - descent + line_gap;
            let scaled_line_height = (raw_line_height * line_height_scale).max(0.0);
            let baseline = cursor_y + ascent;

            let mut glyph_positions = Vec::with_capacity(glyphs.len());
            for mut glyph in glyphs {
                // Each glyph is stored relative to the baseline. Shifting by
                // the computed baseline places it inside the final layout
                // coordinate system where the Y axis points downwards.
                glyph.y += baseline;
                glyph_positions.push(glyph);
            }

            cursor_y += scaled_line_height;

            layout_lines.push(LineData {
                width,
                height: scaled_line_height,
                y: cursor_y - scaled_line_height,
                glyphs: glyph_positions,
            });
        }

        let total_height = cursor_y;
        let total_width = max_line_width;

        let target_width = max_width.unwrap_or(total_width);
        let target_height = max_height.unwrap_or(total_height);

        let vertical_offset = match vertical_align {
            VerticalAlign::Top => 0.0,
            VerticalAlign::Middle => (target_height - total_height) / 2.0,
            VerticalAlign::Bottom => target_height - total_height,
        };

        let mut lines_out = Vec::with_capacity(layout_lines.len());

        for mut line in layout_lines {
            let horizontal_offset = match horizontal_align {
                HorizontalAlign::Left => 0.0,
                HorizontalAlign::Center => (target_width - line.width) / 2.0,
                HorizontalAlign::Right => target_width - line.width,
            };

            if horizontal_offset != 0.0 {
                // Apply the horizontal offset directly to the glyph positions
                // so consumers do not need to perform additional alignment
                // calculations.
                for glyph in &mut line.glyphs {
                    glyph.x += horizontal_offset;
                }
            }

            if vertical_offset != 0.0 {
                // The vertical offset aligns the entire block inside the
                // requested bounding box (top, middle, or bottom).
                for glyph in &mut line.glyphs {
                    glyph.y += vertical_offset;
                }
            }

            lines_out.push(TextLayoutLine {
                line_height: line.height,
                line_width: line.width,
                top: line.y + vertical_offset,
                bottom: line.y + vertical_offset + line.height,
                glyphs: line.glyphs,
            });
        }

        TextLayout {
            config: config.clone(),
            total_height,
            total_width,
            lines: lines_out,
        }
    }

    #[allow(clippy::too_many_arguments)]
    /// Appends glyph fragments to the current line buffer.
    ///
    /// The helper enforces wrapping constraints by measuring the projected
    /// width before committing new fragments. When a limit is exceeded the
    /// current buffer is flushed into the line list, potentially splitting the
    /// provided fragments when hard breaking is enabled.
    fn append_fragments_to_line(
        line_buf: &mut Option<layout_utl::LayoutBuffer<T>>,
        lines: &mut Vec<LineRecord<T>>,
        fragments: &[layout_utl::GlyphFragment<T>],
        max_width: Option<f32>,
        wrap_style: WrapStyle,
        wrap_hard_break: bool,
        font_storage: &mut crate::font_storage::FontStorage,
    ) {
        if fragments.is_empty() {
            return;
        }

        let limit = if matches!(wrap_style, WrapStyle::NoWrap) {
            None
        } else {
            max_width
        };

        let Some(buffer) = layout_utl::LayoutBuffer::from_fragments(fragments, font_storage) else {
            return;
        };

        if let Some(limit_width) = limit {
            if let Some(current) = line_buf.as_mut() {
                let projected = current.projected_concat_length(&buffer, font_storage);
                if projected <= limit_width {
                    current.concat(buffer, font_storage);
                    return;
                }
            }

            if line_buf.is_some() {
                Self::push_line_buffer(line_buf, lines);
            }

            if buffer.width() <= limit_width {
                *line_buf = Some(buffer);
                return;
            }

            if !wrap_hard_break {
                *line_buf = Some(buffer);
                return;
            }

            let mut start = 0usize;
            // Split the fragment into the largest possible chunks that still
            // fit inside the maximum width. Each chunk becomes its own line.
            while start < fragments.len() {
                let mut end = start + 1;
                let mut best =
                    layout_utl::LayoutBuffer::from_fragments(&fragments[start..end], font_storage)
                        .expect("fragment slice must not be empty");

                if best.width() > limit_width {
                    // Even a single glyph cannot fit within the desired width;
                    // emit it as-is so we do not lose information.
                    Self::push_line_buffer(line_buf, lines);
                    *line_buf = Some(best);
                    start = end;
                    continue;
                }

                while end < fragments.len() {
                    // Grow the current best chunk by one glyph at a time. The
                    // incremental projection avoids rebuilding the entire
                    // buffer for each candidate slice.
                    let next_buf = layout_utl::LayoutBuffer::from_fragments(
                        &fragments[end..end + 1],
                        font_storage,
                    )
                    .expect("fragment slice must not be empty");

                    let projected = best.projected_concat_length(&next_buf, font_storage);
                    if projected > limit_width {
                        break;
                    }

                    best.concat(next_buf, font_storage);
                    end += 1;
                }

                Self::push_line_buffer(line_buf, lines);
                *line_buf = Some(best);
                start = end;

                if start < fragments.len() {
                    Self::push_line_buffer(line_buf, lines);
                }
            }
            return;
        }

        if let Some(current) = line_buf.as_mut() {
            current.concat(buffer, font_storage);
        } else {
            *line_buf = Some(buffer);
        }
    }

    /// Adds fragments to the current line while honoring whitespace rules.
    ///
    /// Leading spaces are dropped when they would start a line, mirroring
    /// common text layout behavior. All other fragments are forwarded to the
    /// lower-level append helper.
    fn append_fragments_with_rules(
        line_buf: &mut Option<layout_utl::LayoutBuffer<T>>,
        lines: &mut Vec<LineRecord<T>>,
        fragments: &[layout_utl::GlyphFragment<T>],
        allow_leading_space: bool,
        max_width: Option<f32>,
        wrap_style: WrapStyle,
        wrap_hard_break: bool,
        font_storage: &mut crate::font_storage::FontStorage,
    ) {
        if fragments.is_empty() {
            return;
        }

        if !allow_leading_space
            && let Some(first) = fragments.first()
            && first.ch.is_whitespace()
            && line_buf
                .as_ref()
                .map(|line| line.glyphs.is_empty())
                .unwrap_or(true)
        {
            return;
        }
        Self::append_fragments_to_line(
            line_buf,
            lines,
            fragments,
            max_width,
            wrap_style,
            wrap_hard_break,
            font_storage,
        );
    }

    /// Pushes the buffered line (if any) into the line list.
    ///
    /// Empty lines still carry metrics when the font provides them so the
    /// caller can reserve vertical space for blank rows.
    fn finalize_line(
        line_buf: &mut Option<layout_utl::LayoutBuffer<T>>,
        lines: &mut Vec<LineRecord<T>>,
        metrics: Option<fontdue::LineMetrics>,
    ) {
        if line_buf.is_some() || metrics.is_some() {
            lines.push(LineRecord {
                buffer: line_buf.take(),
                metrics,
            });
        }
    }

    /// Moves the current line buffer into the line list and clears the slot.
    ///
    /// This is used when a wrapping decision forces a break before the incoming
    /// fragments have been appended.
    fn push_line_buffer(
        line_buf: &mut Option<layout_utl::LayoutBuffer<T>>,
        lines: &mut Vec<LineRecord<T>>,
    ) {
        if line_buf.is_some() {
            lines.push(LineRecord {
                buffer: line_buf.take(),
                metrics: None,
            });
        }
    }
}

mod measure_utl {
    //! Legacy measuring helpers kept for reference.

    /// Simplified line statistics produced by `WordBuffer` aggregation.
    pub struct LineBuffer {
        max_ascent: f32,
        max_descent: f32,
        max_line_gap: f32,

        last_char: char,
        instance_length: f32,
        advance_length: f32,
    }

    impl LineBuffer {
        /// Builds a line summary from an aggregated word buffer.
        pub fn from_word_buffer(word: &WordBuffer) -> Self {
            Self {
                max_ascent: 0.0,
                max_descent: 0.0,
                max_line_gap: 0.0,
                last_char: word.last_char,
                instance_length: word.instance_length,
                advance_length: word.advance_length,
            }
        }
    }

    //
    /// Small rolling buffer that accumulates glyph metrics for a single word.
    pub struct WordBuffer {
        first_char: char,
        last_char: char,
        instance_length: f32,
        advance_length: f32,
    }

    impl WordBuffer {
        /// Creates a new word buffer starting with the provided glyph metrics.
        pub fn new(first: char, metrics: &fontdue::Metrics) -> Self {
            Self {
                first_char: first,
                last_char: first,
                instance_length: metrics.width as f32 + metrics.xmin as f32,
                advance_length: metrics.advance_width,
            }
        }

        /// Extends the tracked word with another character.
        ///
        /// The stored lengths are updated so callers can project the space the
        /// word would occupy without re-computing individual glyph metrics.
        pub fn push(&mut self, char: char, metrics: &fontdue::Metrics) {
            self.last_char = char;
            self.instance_length = self.advance_length + metrics.width as f32 + metrics.xmin as f32;
            self.advance_length += metrics.advance_width;
        }

        /// Returns the measured width of the buffered word.
        pub fn length(&self) -> f32 {
            self.instance_length
        }

        /// Predicts the word width if another glyph with `metrics` was appended.
        pub fn length_if_pushed(&self, metrics: &fontdue::Metrics) -> f32 {
            self.advance_length + metrics.width as f32 + metrics.xmin as f32
        }
    }
}

mod layout_utl {
    use crate::font_storage::FontStorage;

    use super::*;
    use std::sync::Arc;

    /// Defines how a character should be handled during layout.
    pub enum CharBehavior {
        /// Always triggers a hard line break (e.g., newline).
        LineBreak,
        /// Breaks a word but may or may not be rendered (e.g., space, tab).
        WordBreak { render_glyph: bool },
        /// Standard character content.
        Regular,
        /// Character should be completely ignored (e.g., non-printable control chars).
        Ignore,
    }

    /// Classifies a character to determine its layout behavior.
    pub fn classify_char(
        ch: char,
        word_separators: &HashSet<char, fxhash::FxBuildHasher>,
        linebreak_char: &HashSet<char, fxhash::FxBuildHasher>,
    ) -> CharBehavior {
        if linebreak_char.contains(&ch) {
            return CharBehavior::LineBreak;
        }

        if word_separators.contains(&ch) {
            // Render the separator only if it is NOT a control character.
            // Spaces are not control chars, but tabs are.
            // This prevents "tofu" rendering for tabs until proper tab support is added.
            return CharBehavior::WordBreak {
                render_glyph: !ch.is_control(),
            };
        }

        if ch.is_control() {
            return CharBehavior::Ignore;
        }

        CharBehavior::Regular
    }

    #[derive(Clone)]
    /// Precomputed glyph data used to build layout buffers.
    ///
    /// Storing the font handle allows kerning to be applied without repeatedly
    /// fetching the same font from storage.
    pub struct GlyphFragment<T> {
        pub ch: char,
        pub glyph_idx: u16,
        pub metrics: fontdue::Metrics,
        pub line_metrics: fontdue::LineMetrics,
        pub font_id: fontdb::ID,
        pub font_size: f32,
        pub font: Arc<fontdue::Font>,
        pub user_data: T,
    }

    /// Buffer of glyph positions with origin located on the baseline.
    ///
    /// Layout buffers are concatenated as new fragments are processed, letting
    /// us calculate kerning-aware widths before the final glyph positions are
    /// produced.
    pub struct LayoutBuffer<T> {
        pub instance_length: f32,

        pub max_accent: f32,
        pub max_descent: f32,
        pub max_line_gap: f32,

        pub first_glyph: u16,
        pub first_font_id: fontdb::ID,
        pub first_font_size: f32,
        pub last_glyph: u16,
        pub last_font_id: fontdb::ID,
        pub last_font_size: f32,
        pub last_metrics: fontdue::Metrics,
        pub last_origin_x: f32,

        pub glyphs: Vec<GlyphPosition<T>>,
    }

    impl<T: Clone> LayoutBuffer<T> {
        /// Creates a buffer containing a single glyph fragment.
        ///
        /// The glyph is stored relative to the baseline so it can be shifted
        /// after all fragments for the line are known.
        pub fn new(
            glyph_idx: u16,
            metrics: &fontdue::Metrics,
            line_metrics: &fontdue::LineMetrics,
            font_id: fontdb::ID,
            font_size: f32,
            user_data: T,
        ) -> Self {
            let mut buffer = Self {
                instance_length: metrics.width as f32 + metrics.xmin as f32,
                max_accent: line_metrics.ascent,
                max_descent: line_metrics.descent,
                max_line_gap: line_metrics.line_gap,
                first_glyph: glyph_idx,
                first_font_id: font_id,
                first_font_size: font_size,
                last_glyph: glyph_idx,
                last_font_id: font_id,
                last_font_size: font_size,
                last_metrics: *metrics,
                last_origin_x: 0.0,
                glyphs: vec![],
            };

            buffer.glyphs.push(GlyphPosition {
                glyph_id: GlyphId::new(font_id, glyph_idx, font_size),
                x: metrics.xmin as f32,
                y: -(metrics.ymin as f32 + metrics.height as f32),
                user_data,
            });

            buffer
        }

        /// Appends another glyph to the buffer, updating metrics and kerning.
        ///
        /// The kerning calculation uses the provided font handle when the
        /// previous and new glyph share the same font and size. This keeps the
        /// layout accurate while avoiding redundant lookups.
        pub fn push(
            &mut self,
            glyph_idx: u16,
            metrics: &fontdue::Metrics,
            line_metrics: &fontdue::LineMetrics,
            font: &fontdue::Font,
            font_id: fontdb::ID,
            font_size: f32,
            user_data: T,
            _font_storage: &mut FontStorage,
        ) {
            let advance_kerned = if self.last_font_id == font_id
                && (self.last_font_size - font_size).abs() < f32::EPSILON
            {
                let kerning = font
                    .horizontal_kern_indexed(self.last_glyph, glyph_idx, font_size)
                    .unwrap_or(0.0);
                self.last_metrics.advance_width + kerning
            } else {
                // for simplicity, just ignore kerning for different font or size
                /*
                // use average kerning for different font or size

                let kerning_of_curr_font = font
                    .horizontal_kern_indexed(self.last_glyph, glyph_idx, font_size)
                    .unwrap_or(0.0);
                let kerning_of_prev_font = font_storage
                    .font(self.last_font_id)
                    .and_then(|f| {
                        f.horizontal_kern_indexed(self.last_glyph, glyph_idx, self.last_font_size)
                    })
                    .unwrap_or(0.0);

                let average_kerning = (kerning_of_curr_font + kerning_of_prev_font) / 2.0;

                self.last_metrics.advance_width + average_kerning
                */

                self.last_metrics.advance_width
            };

            let new_origin_x = self.last_origin_x + advance_kerned;

            self.instance_length = new_origin_x + metrics.width as f32 + metrics.xmin as f32;
            self.max_accent = self.max_accent.max(line_metrics.ascent);
            self.max_descent = self.max_descent.max(line_metrics.descent);
            self.max_line_gap = self.max_line_gap.max(line_metrics.line_gap);
            self.last_glyph = glyph_idx;
            self.last_font_id = font_id;
            self.last_font_size = font_size;
            self.last_metrics = *metrics;
            self.last_origin_x = new_origin_x;
            self.glyphs.push(GlyphPosition {
                glyph_id: GlyphId::new(font_id, glyph_idx, font_size),
                x: new_origin_x + metrics.xmin as f32,
                y: -(metrics.ymin as f32 + metrics.height as f32),
                user_data,
            });
        }

        /// Concatenates another layout buffer, adjusting positions in-place.
        ///
        /// When the buffers originate from the same font and size we apply
        /// kerning between the boundary glyphs; otherwise the buffers are joined
        /// using the recorded advance of the current buffer.
        pub fn concat(&mut self, other: LayoutBuffer<T>, font_storage: &mut FontStorage) {
            let advance_kerned = if self.last_font_id == other.first_font_id
                && (self.last_font_size - other.first_font_size).abs() < f32::EPSILON
            {
                let font = font_storage
                    .font(self.last_font_id)
                    .expect("font must exist in font storage");
                let kerning = font
                    .horizontal_kern_indexed(
                        self.last_glyph,
                        other.first_glyph,
                        self.last_font_size,
                    )
                    .unwrap_or(0.0);
                self.last_metrics.advance_width + kerning
            } else {
                // for simplicity, just ignore kerning for different font or size
                self.last_metrics.advance_width
            };

            let x_offset = self.last_origin_x + advance_kerned;

            let new_instance_length = x_offset + other.instance_length;
            let new_origin_x = x_offset + other.last_origin_x;

            self.instance_length = new_instance_length;
            self.max_accent = self.max_accent.max(other.max_accent);
            self.max_descent = self.max_descent.max(other.max_descent);
            self.max_line_gap = self.max_line_gap.max(other.max_line_gap);
            self.last_glyph = other.last_glyph;
            self.last_font_id = other.last_font_id;
            self.last_font_size = other.last_font_size;
            self.last_metrics = other.last_metrics;
            self.last_origin_x = new_origin_x;
            for mut glyph_pos in other.glyphs {
                glyph_pos.x += x_offset;
                self.glyphs.push(glyph_pos);
            }
        }

        /// Returns the current width of the buffer.
        pub fn width(&self) -> f32 {
            self.instance_length.max(0.0)
        }

        /// Estimates the width after concatenating `other` without modifying `self`.
        ///
        /// This prediction is used during wrapping decisions to avoid expensive
        /// cloning or re-layout work.
        pub fn projected_concat_length(
            &self,
            other: &LayoutBuffer<T>,
            font_storage: &mut FontStorage,
        ) -> f32 {
            let advance_kerned = if self.last_font_id == other.first_font_id
                && (self.last_font_size - other.first_font_size).abs() < f32::EPSILON
            {
                font_storage
                    .font(self.last_font_id)
                    .and_then(|font| {
                        font.horizontal_kern_indexed(
                            self.last_glyph,
                            other.first_glyph,
                            self.last_font_size,
                        )
                    })
                    .unwrap_or(0.0)
                    + self.last_metrics.advance_width
            } else {
                self.last_metrics.advance_width
            };

            let x_offset = self.last_origin_x + advance_kerned;
            x_offset + other.instance_length
        }

        /// Returns line metrics derived from the buffered glyph fragments.
        pub fn line_metrics(&self) -> (f32, f32, f32) {
            (self.max_accent, self.max_descent, self.max_line_gap)
        }

        /// Builds a layout buffer from a slice of glyph fragments.
        ///
        /// `None` is returned when the slice is empty because there are no
        /// glyphs to measure or position.
        pub fn from_fragments(
            fragments: &[GlyphFragment<T>],
            font_storage: &mut FontStorage,
        ) -> Option<LayoutBuffer<T>> {
            let first = fragments.first()?;
            let mut buffer = LayoutBuffer::new(
                first.glyph_idx,
                &first.metrics,
                &first.line_metrics,
                first.font_id,
                first.font_size,
                first.user_data.clone(),
            );

            for fragment in fragments.iter().skip(1) {
                buffer.push(
                    fragment.glyph_idx,
                    &fragment.metrics,
                    &fragment.line_metrics,
                    fragment.font.as_ref(),
                    fragment.font_id,
                    fragment.font_size,
                    fragment.user_data.clone(),
                    font_storage,
                );
            }

            Some(buffer)
        }
    }
}
