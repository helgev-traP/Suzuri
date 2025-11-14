use std::collections::HashSet;

use crate::glyph_id::GlyphId;

pub struct TextData {
    pub texts: Vec<TextElement>,
}

pub struct TextElement {
    pub font_id: fontdb::ID,
    pub font_size: f32,
    pub content: String,
}

pub struct TextLayoutConfig {
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub horizontal_align: HorizontalAlign,
    pub vertical_align: VerticalAlign,
    pub line_height_scale: f32,
    pub wrap_style: WrapStyle,
    pub wrap_hard_break: bool,
    pub word_separators: HashSet<char, fxhash::FxBuildHasher>,
    pub linebreak_char: HashSet<char, fxhash::FxBuildHasher>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HorizontalAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerticalAlign {
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WrapStyle {
    NoWrap,
    WordWrap,
    CharWrap,
}

pub struct TextLayout {
    pub total_height: f32,
    pub total_width: f32,
    pub lines: Vec<TextLayoutLine>,
}

pub struct TextLayoutLine {
    pub line_height: f32,
    pub line_width: f32,
    pub glyphs: Vec<GlyphPosition>,
}

/// **Y-axis goes down**
pub struct GlyphPosition {
    pub glyph_id: GlyphId,
    pub x: f32,
    pub y: f32,
}

struct LineRecord {
    buffer: Option<layout::LayoutBuffer>,
    metrics: Option<fontdue::LineMetrics>,
}

impl Default for TextData {
    fn default() -> Self {
        Self::new()
    }
}

impl TextData {
    pub fn new() -> Self {
        Self { texts: vec![] }
    }

    pub fn append(&mut self, text: TextElement) {
        self.texts.push(text);
    }

    pub fn clear(&mut self) {
        self.texts.clear();
    }
}

impl TextData {
    pub fn measure(
        &self,
        config: &TextLayoutConfig,
        font_storage: &mut crate::font_storage::FontStorage,
    ) -> [f32; 2] {
        let layout = self.layout(config, font_storage);
        [layout.total_width, layout.total_height]
    }

    pub fn layout(
        &self,
        config: &TextLayoutConfig,
        font_storage: &mut crate::font_storage::FontStorage,
    ) -> TextLayout {
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

        struct LineData {
            width: f32,
            height: f32,
            glyphs: Vec<GlyphPosition>,
        }

        let mut lines: Vec<LineRecord> = Vec::new();
        let mut line_buf: Option<layout::LayoutBuffer> = None;
        let mut word_buf: Option<Vec<layout::GlyphFragment>> = None;
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

            for ch in text.content.chars() {
                let glyph_idx = font.lookup_glyph_index(ch);
                let metrics = font.metrics_indexed(glyph_idx, text.font_size);
                let fragment = layout::GlyphFragment {
                    ch,
                    glyph_idx,
                    metrics,
                    line_metrics: line_metric,
                    font_id: text.font_id,
                    font_size: text.font_size,
                    font: Arc::clone(&font),
                };

                if linebreak_char.contains(&ch) {
                    // Newline characters always terminate the current line. If the
                    // font provides a glyph we keep it so the renderer can emulate
                    // visible line break marks when desired.
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
                    Self::finalize_line(&mut line_buf, &mut lines, Some(line_metric));
                    continue;
                }

                if word_separators.contains(&ch) {
                    // Preserve the separator so the caller can render the
                    // whitespace but keep wrapping decisions at word
                    // boundaries.
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
                    continue;
                }

                if matches!(wrap_style, WrapStyle::CharWrap) {
                    // Each character is flushed immediately so the helper only
                    // operates on single glyph fragments.
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
                    continue;
                }

                match &mut word_buf {
                    Some(buffer) => buffer.push(fragment),
                    None => word_buf = Some(vec![fragment]),
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

        let mut layout_lines: Vec<LineData> = Vec::new();
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
                glyphs: line.glyphs,
            });
        }

        TextLayout {
            total_height: total_height.max(target_height),
            total_width: total_width.max(target_width),
            lines: lines_out,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn append_fragments_to_line(
        line_buf: &mut Option<layout::LayoutBuffer>,
        lines: &mut Vec<LineRecord>,
        fragments: &[layout::GlyphFragment],
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

        let Some(buffer) = layout::LayoutBuffer::from_fragments(fragments, font_storage) else {
            return;
        };

        if let Some(limit_width) = limit {
            if let Some(projected) = line_buf
                .as_ref()
                .map(|current| current.projected_concat_length(&buffer, font_storage))
            {
                if projected <= limit_width {
                    if let Some(current) = line_buf.as_mut() {
                        current.concat(buffer, font_storage);
                    }
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
                    layout::LayoutBuffer::from_fragments(&fragments[start..end], font_storage)
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
                    let candidate =
                        layout::LayoutBuffer::from_fragments(&fragments[start..=end], font_storage)
                            .expect("fragment slice must not be empty");

                    if candidate.width() > limit_width {
                        break;
                    }

                    best = candidate;
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

    fn append_fragments_with_rules(
        line_buf: &mut Option<layout::LayoutBuffer>,
        lines: &mut Vec<LineRecord>,
        fragments: &[layout::GlyphFragment],
        allow_leading_space: bool,
        max_width: Option<f32>,
        wrap_style: WrapStyle,
        wrap_hard_break: bool,
        font_storage: &mut crate::font_storage::FontStorage,
    ) {
        if fragments.is_empty() {
            return;
        }

        if !allow_leading_space {
            if let Some(first) = fragments.first() {
                if first.ch.is_whitespace()
                    && line_buf
                        .as_ref()
                        .map(|line| line.glyphs.is_empty())
                        .unwrap_or(true)
                {
                    return;
                }
            }
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

    fn finalize_line(
        line_buf: &mut Option<layout::LayoutBuffer>,
        lines: &mut Vec<LineRecord>,
        metrics: Option<fontdue::LineMetrics>,
    ) {
        if line_buf.is_some() || metrics.is_some() {
            lines.push(LineRecord {
                buffer: line_buf.take(),
                metrics,
            });
        }
    }

    fn push_line_buffer(line_buf: &mut Option<layout::LayoutBuffer>, lines: &mut Vec<LineRecord>) {
        if line_buf.is_some() {
            lines.push(LineRecord {
                buffer: line_buf.take(),
                metrics: None,
            });
        }
    }
}

#[allow(dead_code)]
mod measure {
    pub struct LineBuffer {
        max_ascent: f32,
        max_descent: f32,
        max_line_gap: f32,

        last_char: char,
        instance_length: f32,
        advance_length: f32,
    }

    impl LineBuffer {
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
    pub struct WordBuffer {
        first_char: char,
        last_char: char,
        instance_length: f32,
        advance_length: f32,
    }

    impl WordBuffer {
        pub fn new(first: char, metrics: &fontdue::Metrics) -> Self {
            Self {
                first_char: first,
                last_char: first,
                instance_length: metrics.width as f32 + metrics.xmin as f32,
                advance_length: metrics.advance_width,
            }
        }

        pub fn push(&mut self, char: char, metrics: &fontdue::Metrics) {
            self.last_char = char;
            self.instance_length = self.advance_length + metrics.width as f32 + metrics.xmin as f32;
            self.advance_length += metrics.advance_width;
        }

        pub fn length(&self) -> f32 {
            self.instance_length
        }

        pub fn length_if_pushed(&self, metrics: &fontdue::Metrics) -> f32 {
            self.advance_length + metrics.width as f32 + metrics.xmin as f32
        }
    }
}

mod layout {
    use crate::font_storage::FontStorage;

    use super::*;
    use std::sync::Arc;

    #[derive(Clone)]
    pub struct GlyphFragment {
        pub ch: char,
        pub glyph_idx: u16,
        pub metrics: fontdue::Metrics,
        pub line_metrics: fontdue::LineMetrics,
        pub font_id: fontdb::ID,
        pub font_size: f32,
        pub font: Arc<fontdue::Font>,
    }

    /// Y-Origin at the baseline.
    pub struct LayoutBuffer {
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

        pub glyphs: Vec<GlyphPosition>,
    }

    impl LayoutBuffer {
        pub fn new(
            glyph_idx: u16,
            metrics: &fontdue::Metrics,
            line_metrics: &fontdue::LineMetrics,
            font_id: fontdb::ID,
            font_size: f32,
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
            });

            buffer
        }

        pub fn push(
            &mut self,
            glyph_idx: u16,
            metrics: &fontdue::Metrics,
            line_metrics: &fontdue::LineMetrics,
            font: &fontdue::Font,
            font_id: fontdb::ID,
            font_size: f32,
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
            });
        }

        pub fn concat(&mut self, other: LayoutBuffer, font_storage: &mut FontStorage) {
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

        pub fn width(&self) -> f32 {
            self.instance_length.max(0.0)
        }

        pub fn projected_concat_length(
            &self,
            other: &LayoutBuffer,
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

        pub fn line_metrics(&self) -> (f32, f32, f32) {
            (self.max_accent, self.max_descent, self.max_line_gap)
        }

        pub fn from_fragments(
            fragments: &[GlyphFragment],
            font_storage: &mut FontStorage,
        ) -> Option<LayoutBuffer> {
            let first = fragments.first()?;
            let mut buffer = LayoutBuffer::new(
                first.glyph_idx,
                &first.metrics,
                &first.line_metrics,
                first.font_id,
                first.font_size,
            );

            for fragment in fragments.iter().skip(1) {
                buffer.push(
                    fragment.glyph_idx,
                    &fragment.metrics,
                    &fragment.line_metrics,
                    fragment.font.as_ref(),
                    fragment.font_id,
                    fragment.font_size,
                    font_storage,
                );
            }

            Some(buffer)
        }
    }
}
