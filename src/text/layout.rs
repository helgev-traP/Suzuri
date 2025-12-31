use core::f32;

use crate::{font_storage, glyph_id::GlyphId, text::TextData};

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
    /// Number of spaces to treat a tab character as.
    pub tab_size_in_spaces: usize,
}

impl Default for TextLayoutConfig {
    fn default() -> Self {
        Self {
            max_width: None,
            max_height: None,
            horizontal_align: HorizontalAlign::Left,
            vertical_align: VerticalAlign::Top,
            line_height_scale: 1.0,
            wrap_style: WrapStyle::WordWrap,
            tab_size_in_spaces: 4,
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
    /// The metrics of the glyph.
    pub glyph_metrics: fontdue::Metrics,
    /// The absolute X coordinate of the upper left corner of the glyph.
    pub x: f32,
    /// The absolute Y coordinate of the upper left corner of the glyph.
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
        let mut context = LayoutContext::new(config);

        // for all texts
        for text in &self.texts {
            // get font info
            let font_id = text.font_id;
            let Some(font) = font_storage.font(font_id) else {
                continue;
            };
            let font_size = text.font_size;
            let Some(line_metrics) = font.horizontal_line_metrics(font_size) else {
                unimplemented!("vertical text layout is not supported yet");
            };
            let user_data = &text.user_data;

            for ch in text.content.chars() {
                let glyph_idx = font.lookup_glyph_index(ch);

                match ch {
                    '\n' | '\u{2028}' | '\u{2029}' => {
                        context.handle_newline(font_storage);
                    }
                    ' ' => {
                        context.handle_space(
                            glyph_idx,
                            &font,
                            font_id,
                            font_size,
                            line_metrics,
                            user_data.clone(),
                            font_storage,
                        );
                    }
                    '\t' => {
                        context.handle_tab(&font, font_size, line_metrics, font_storage);
                    }
                    _ => {
                        context.handle_char(
                            glyph_idx,
                            &font,
                            font_id,
                            font_size,
                            line_metrics,
                            user_data.clone(),
                        );
                    }
                }
            }
        }

        context.flush(font_storage);
        let lines = context.lines;

        // Calculate total dimensions
        let mut total_height = 0.0;
        let mut max_line_width = 0.0f32;

        struct ProcessedLine<T> {
            fragment: LayoutFragment<T>,
            line_height: f32,
            ascent: f32,
            descent: f32,
        }

        let mut processed_lines = Vec::with_capacity(lines.len());

        for fragment in lines {
            let width = fragment.instance_length;
            // ascent is max_accent (typo in struct), descent is max_descent
            let ascent = fragment.max_ascent;
            let descent = fragment.max_descent;
            let line_gap = fragment.max_line_gap;

            // Note: descent is typically negative.
            let content_height = ascent - descent;
            let original_line_height = content_height + line_gap;
            let line_height = original_line_height * config.line_height_scale;

            max_line_width = max_line_width.max(width);
            total_height += line_height;

            processed_lines.push(ProcessedLine {
                fragment,
                line_height,
                ascent,
                descent,
            });
        }

        let container_width = config.max_width.unwrap_or(max_line_width);
        // Vertical align setup
        let layout_height = config.max_height.unwrap_or(total_height);
        let mut current_y = match config.vertical_align {
            VerticalAlign::Top => 0.0,
            VerticalAlign::Middle => (layout_height - total_height) / 2.0,
            VerticalAlign::Bottom => layout_height - total_height,
        };

        // Final assembly
        let mut final_lines = Vec::with_capacity(processed_lines.len());

        for line in processed_lines {
            // Horizontal Alignment
            let x_offset = match config.horizontal_align {
                HorizontalAlign::Left => 0.0,
                // If container_width < width (overflow), this might be negative.
                HorizontalAlign::Center => (container_width - line.fragment.instance_length) / 2.0,
                HorizontalAlign::Right => container_width - line.fragment.instance_length,
            };

            // Vertical positioning
            // Baseline Y for this line
            // Center text vertically within the scaled line height
            let baseline_y =
                current_y + (line.line_height / 2.0) - ((line.ascent + line.descent) / 2.0);

            let mut final_glyphs = Vec::with_capacity(line.fragment.buffer.len());
            for mut glyph in line.fragment.buffer {
                glyph.x += x_offset;
                glyph.y += baseline_y;
                final_glyphs.push(glyph);
            }

            final_lines.push(TextLayoutLine {
                line_height: line.line_height,
                line_width: line.fragment.instance_length,
                top: current_y,
                bottom: current_y + line.line_height,
                glyphs: final_glyphs,
            });

            current_y += line.line_height;
        }

        TextLayout {
            config: config.clone(),
            total_height,
            total_width: max_line_width,
            lines: final_lines,
        }
    }
}

struct LayoutContext<'a, T> {
    config: &'a TextLayoutConfig,
    lines: Vec<LayoutFragment<T>>,
    line_fragment: Option<LayoutFragment<T>>,
    word_fragment: Option<LayoutFragment<T>>,
}

impl<'a, T: Clone> LayoutContext<'a, T> {
    fn new(config: &'a TextLayoutConfig) -> Self {
        Self {
            config,
            lines: Vec::new(),
            line_fragment: None,
            word_fragment: None,
        }
    }

    fn handle_newline(&mut self, font_storage: &mut font_storage::FontStorage) {
        match (self.line_fragment.take(), self.word_fragment.take()) {
            (Some(mut lf), Some(mut wf)) => {
                if lf.try_concat_in_length(
                    &mut wf,
                    font_storage,
                    self.config.max_width.unwrap_or(f32::INFINITY),
                ) {
                    self.lines.push(lf);
                } else {
                    self.lines.push(lf);
                    self.lines.push(wf);
                }
            }
            (Some(fragment), None) | (None, Some(fragment)) => {
                self.lines.push(fragment);
            }
            (None, None) => {
                self.lines.push(LayoutFragment::new_blank());
            }
        }
    }

    fn handle_space(
        &mut self,
        glyph_idx: u16,
        font: &fontdue::Font,
        font_id: fontdb::ID,
        font_size: f32,
        line_metrics: fontdue::LineMetrics,
        user_data: T,
        font_storage: &mut font_storage::FontStorage,
    ) {
        match (
            self.config.wrap_style,
            self.line_fragment.take(),
            self.word_fragment.take(),
        ) {
            (WrapStyle::WordWrap, Some(mut lf), Some(mut wf)) => {
                if lf.try_concat_in_length(
                    &mut wf,
                    font_storage,
                    self.config.max_width.unwrap_or(f32::INFINITY),
                ) {
                    lf.push_char(
                        glyph_idx,
                        font,
                        font_id,
                        font_size,
                        line_metrics,
                        user_data.clone(),
                    );
                    self.line_fragment = Some(lf);
                } else {
                    self.lines.push(lf);
                    wf.push_char(
                        glyph_idx,
                        font,
                        font_id,
                        font_size,
                        line_metrics,
                        user_data.clone(),
                    );
                    self.line_fragment = Some(wf);
                }
            }
            (WrapStyle::WordWrap, Some(mut lf), None) => {
                lf.push_char(
                    glyph_idx,
                    font,
                    font_id,
                    font_size,
                    line_metrics,
                    user_data.clone(),
                );
                self.line_fragment = Some(lf);
            }
            (WrapStyle::WordWrap, None, Some(mut wf)) => {
                wf.push_char(
                    glyph_idx,
                    font,
                    font_id,
                    font_size,
                    line_metrics,
                    user_data.clone(),
                );
                self.line_fragment = Some(wf);
            }
            (WrapStyle::WordWrap, None, None) => {
                self.line_fragment = Some(LayoutFragment::new(
                    glyph_idx,
                    font,
                    font_id,
                    font_size,
                    line_metrics,
                    user_data.clone(),
                ));
            }

            (WrapStyle::CharWrap, Some(mut lf), _) | (WrapStyle::NoWrap, Some(mut lf), _) => {
                lf.push_char(
                    glyph_idx,
                    font,
                    font_id,
                    font_size,
                    line_metrics,
                    user_data.clone(),
                );
                self.line_fragment = Some(lf);
            }
            (WrapStyle::CharWrap, None, _) | (WrapStyle::NoWrap, None, _) => {
                self.line_fragment = Some(LayoutFragment::new(
                    glyph_idx,
                    font,
                    font_id,
                    font_size,
                    line_metrics,
                    user_data.clone(),
                ));
            }
        }
    }

    fn handle_tab(
        &mut self,
        font: &fontdue::Font,
        font_size: f32,
        line_metrics: fontdue::LineMetrics,
        font_storage: &mut font_storage::FontStorage,
    ) {
        let space_size = font.metrics(' ', font_size).advance_width;
        let tab_size = space_size * self.config.tab_size_in_spaces as f32;

        if let Some(mut wf) = self.word_fragment.take() {
            if let Some(mut lf) = self.line_fragment.take() {
                if lf.try_concat_in_length(
                    &mut wf,
                    font_storage,
                    self.config.max_width.unwrap_or(f32::INFINITY),
                ) {
                    self.line_fragment = Some(lf);
                } else {
                    self.lines.push(lf);
                    self.line_fragment = Some(wf);
                }
            } else {
                self.line_fragment = Some(wf);
            }
        }

        if let Some(lf) = &mut self.line_fragment {
            lf.instance_length += tab_size;
            lf.next_origin_x += tab_size;
            lf.max_ascent = lf.max_ascent.max(line_metrics.ascent);
            lf.max_descent = lf.max_descent.max(line_metrics.descent);
            lf.max_line_gap = lf.max_line_gap.max(line_metrics.line_gap);
        } else {
            let mut lf = LayoutFragment::new_blank();
            lf.instance_length = tab_size;
            lf.next_origin_x = tab_size;
            lf.max_ascent = line_metrics.ascent;
            lf.max_descent = line_metrics.descent;
            lf.max_line_gap = line_metrics.line_gap;
            self.line_fragment = Some(lf);
        }
    }

    fn handle_char(
        &mut self,
        glyph_idx: u16,
        font: &fontdue::Font,
        font_id: fontdb::ID,
        font_size: f32,
        line_metrics: fontdue::LineMetrics,
        user_data: T,
    ) {
        match (
            self.config.wrap_style,
            self.line_fragment.take(),
            self.word_fragment.take(),
        ) {
            (WrapStyle::WordWrap, Some(lf), Some(mut wf)) => {
                let w_result = wf.try_push_char_in_length(
                    glyph_idx,
                    font,
                    font_id,
                    font_size,
                    line_metrics,
                    user_data.clone(),
                    self.config.max_width.unwrap_or(f32::INFINITY),
                );
                if w_result {
                    self.word_fragment = Some(wf);
                    self.line_fragment = Some(lf);
                } else {
                    self.lines.push(lf);
                    self.lines.push(wf);
                    self.word_fragment = Some(LayoutFragment::new(
                        glyph_idx,
                        font,
                        font_id,
                        font_size,
                        line_metrics,
                        user_data.clone(),
                    ));
                }
            }
            (WrapStyle::WordWrap, Some(lf), None) => {
                self.word_fragment = Some(LayoutFragment::new(
                    glyph_idx,
                    font,
                    font_id,
                    font_size,
                    line_metrics,
                    user_data.clone(),
                ));
                self.line_fragment = Some(lf);
            }
            (WrapStyle::WordWrap, None, Some(mut wf)) => {
                wf.push_char(
                    glyph_idx,
                    font,
                    font_id,
                    font_size,
                    line_metrics,
                    user_data.clone(),
                );
                self.line_fragment = Some(wf);
            }
            (WrapStyle::WordWrap, None, None) => {
                self.line_fragment = Some(LayoutFragment::new(
                    glyph_idx,
                    font,
                    font_id,
                    font_size,
                    line_metrics,
                    user_data.clone(),
                ));
            }

            (WrapStyle::CharWrap, Some(mut lf), _) => {
                let result = lf.try_push_char_in_length(
                    glyph_idx,
                    font,
                    font_id,
                    font_size,
                    line_metrics,
                    user_data.clone(),
                    self.config.max_width.unwrap_or(f32::INFINITY),
                );

                if result {
                    self.line_fragment = Some(lf);
                } else {
                    self.lines.push(lf);
                    self.line_fragment = Some(LayoutFragment::new(
                        glyph_idx,
                        font,
                        font_id,
                        font_size,
                        line_metrics,
                        user_data.clone(),
                    ));
                }
            }
            (WrapStyle::CharWrap, None, _) => {
                self.line_fragment = Some(LayoutFragment::new(
                    glyph_idx,
                    font,
                    font_id,
                    font_size,
                    line_metrics,
                    user_data.clone(),
                ));
            }

            (WrapStyle::NoWrap, Some(mut lf), _) => {
                lf.push_char(
                    glyph_idx,
                    font,
                    font_id,
                    font_size,
                    line_metrics,
                    user_data.clone(),
                );
                self.line_fragment = Some(lf);
            }
            (WrapStyle::NoWrap, None, _) => {
                self.line_fragment = Some(LayoutFragment::new(
                    glyph_idx,
                    font,
                    font_id,
                    font_size,
                    line_metrics,
                    user_data.clone(),
                ));
            }
        }
    }

    fn flush(&mut self, font_storage: &mut font_storage::FontStorage) {
        match (
            self.config.wrap_style,
            self.word_fragment.take(),
            self.line_fragment.take(),
        ) {
            (WrapStyle::WordWrap, None, Some(ragment))
            | (WrapStyle::WordWrap, Some(ragment), None) => {
                self.lines.push(ragment);
            }
            (WrapStyle::WordWrap, Some(mut lf), Some(mut wf)) => {
                let result = lf.try_concat_in_length(
                    &mut wf,
                    font_storage,
                    self.config.max_width.unwrap_or(f32::INFINITY),
                );

                if result {
                    self.lines.push(lf);
                } else {
                    self.lines.push(lf);
                    self.lines.push(wf);
                }
            }

            (WrapStyle::CharWrap, Some(fragment), _) => {
                self.lines.push(fragment);
            }

            (WrapStyle::NoWrap, Some(fragment), _) => {
                self.lines.push(fragment);
            }

            (WrapStyle::CharWrap, None, _) => (),
            (WrapStyle::NoWrap, None, _) => (),
            (_, None, None) => (),
        }
    }
}

struct LayoutFragment<T> {
    buffer: Vec<GlyphPosition<T>>,

    instance_length: f32,

    max_ascent: f32,
    max_descent: f32,
    max_line_gap: f32,

    next_origin_x: f32,
}

impl<T> LayoutFragment<T> {
    #[inline(always)]
    fn new(
        glyph_idx: u16,
        font: &fontdue::Font,
        font_id: fontdb::ID,
        size: f32,
        line_metrics: fontdue::LineMetrics,
        user_data: T,
    ) -> Self {
        let id = GlyphId::new(font_id, glyph_idx, size);
        let metrics = font.metrics_indexed(glyph_idx, size);

        Self {
            buffer: vec![GlyphPosition {
                glyph_id: id,
                glyph_metrics: metrics,
                x: metrics.xmin as f32,
                y: -(metrics.ymin as f32 + metrics.height as f32),
                user_data,
            }],
            instance_length: (metrics.xmin + metrics.width as i32) as f32,
            max_ascent: line_metrics.ascent,
            max_descent: line_metrics.descent,
            max_line_gap: line_metrics.line_gap,
            next_origin_x: metrics.advance_width,
        }
    }

    #[inline(always)]
    fn new_blank() -> Self {
        Self {
            buffer: Vec::new(),
            instance_length: 0.0,
            max_ascent: 0.0,
            max_descent: 0.0,
            max_line_gap: 0.0,
            next_origin_x: 0.0,
        }
    }

    #[inline(always)]
    fn push_char(
        &mut self,
        glyph_idx: u16,
        font: &fontdue::Font,
        font_id: fontdb::ID,
        size: f32,
        line_metrics: fontdue::LineMetrics,
        user_data: T,
    ) {
        let id = GlyphId::new(font_id, glyph_idx, size);
        let metrics = font.metrics_indexed(glyph_idx, size);

        let x_kern = self
            .buffer
            .last()
            .and_then(|left| {
                // Ignore kerning if font id is different
                if left.glyph_id.font_id() == font_id {
                    Some(left.glyph_id.glyph_index())
                } else {
                    None
                }
            })
            .and_then(|left_idx| font.horizontal_kern_indexed(left_idx, glyph_idx, size))
            .unwrap_or(0.0);

        // fix x position
        self.next_origin_x += x_kern;

        self.buffer.push(GlyphPosition {
            glyph_id: id,
            glyph_metrics: metrics,
            x: self.next_origin_x + metrics.xmin as f32,
            y: -(metrics.ymin as f32 + metrics.height as f32),
            user_data,
        });

        self.instance_length = self
            .instance_length
            .max(self.next_origin_x + metrics.xmin as f32 + metrics.width as f32);
        self.max_ascent = self.max_ascent.max(line_metrics.ascent);
        self.max_descent = self.max_descent.max(line_metrics.descent);
        self.max_line_gap = self.max_line_gap.max(line_metrics.line_gap);
        self.next_origin_x += metrics.advance_width;
    }

    /// try to push char to the fragment, returns true if the char is pushed to the fragment, false if the char will overflow the max length and the char is not pushed
    #[inline(always)]
    fn try_push_char_in_length(
        &mut self,
        glyph_idx: u16,
        font: &fontdue::Font,
        font_id: fontdb::ID,
        size: f32,
        line_metrics: fontdue::LineMetrics,
        user_data: T,
        max_length: f32,
    ) -> bool {
        let id = GlyphId::new(font_id, glyph_idx, size);
        let metrics = font.metrics_indexed(glyph_idx, size);

        let x_kern = self
            .buffer
            .last()
            .and_then(|left| {
                // Ignore kerning if font id is different
                if left.glyph_id.font_id() == font_id {
                    Some(left.glyph_id.glyph_index())
                } else {
                    None
                }
            })
            .and_then(|left_idx| font.horizontal_kern_indexed(left_idx, glyph_idx, size))
            .unwrap_or(0.0);

        // fix x position
        let fixed_next_origin_x = self.next_origin_x + x_kern;

        if fixed_next_origin_x + metrics.xmin as f32 + metrics.width as f32 > max_length {
            return false;
        }

        self.buffer.push(GlyphPosition {
            glyph_id: id,
            glyph_metrics: metrics,
            x: fixed_next_origin_x + metrics.xmin as f32,
            y: -(metrics.ymin as f32 + metrics.height as f32),
            user_data,
        });

        self.instance_length = self
            .instance_length
            .max(fixed_next_origin_x + metrics.xmin as f32 + metrics.width as f32);
        self.max_ascent = self.max_ascent.max(line_metrics.ascent);
        self.max_descent = self.max_descent.max(line_metrics.descent);
        self.max_line_gap = self.max_line_gap.max(line_metrics.line_gap);
        self.next_origin_x = fixed_next_origin_x + metrics.advance_width;

        true
    }

    /// try to concat other fragment to self, returns true if the fragment is pushed to the line, false if the fragment will overflow the max length and the fragment is not pushed
    #[inline(always)]
    fn try_concat_in_length(
        &mut self,
        other: &mut Self,
        font_storage: &mut font_storage::FontStorage,
        max_length: f32,
    ) -> bool {
        let x_kern = if let Some(last_glyph_of_self) = self.buffer.last() {
            if let Some(first_glyph_of_other) = other.buffer.first() {
                if (last_glyph_of_self.glyph_id.font_id()
                    == first_glyph_of_other.glyph_id.font_id())
                    && (last_glyph_of_self.glyph_id.font_size()
                        == first_glyph_of_other.glyph_id.font_size())
                {
                    font_storage
                        .font(last_glyph_of_self.glyph_id.font_id())
                        .and_then(|font| {
                            font.horizontal_kern_indexed(
                                last_glyph_of_self.glyph_id.glyph_index(),
                                first_glyph_of_other.glyph_id.glyph_index(),
                                last_glyph_of_self.glyph_id.font_size(),
                            )
                        })
                        .unwrap_or(0.0)
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0
        };

        // fix x position
        let fixed_next_origin_x = self.next_origin_x + x_kern;

        let new_instance_length = fixed_next_origin_x + other.instance_length;
        if new_instance_length > max_length {
            return false;
        }

        for glyph in &mut other.buffer {
            glyph.x += fixed_next_origin_x;
        }
        self.buffer.append(&mut other.buffer);

        // update info
        self.instance_length = self.instance_length.max(new_instance_length);
        self.max_ascent = self.max_ascent.max(other.max_ascent);
        self.max_descent = self.max_descent.max(other.max_descent);
        self.max_line_gap = self.max_line_gap.max(other.max_line_gap);
        self.next_origin_x = fixed_next_origin_x + other.next_origin_x;

        true
    }
}
