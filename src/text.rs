use crate::glyph_id::GlyphId;

pub struct TextData {
    pub texts: Vec<TextElement>,
}

pub struct TextElement {
    pub font_id: fontdb::ID,
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
}

pub enum HorizontalAlign {
    Left,
    Center,
    Right,
}

pub enum VerticalAlign {
    Top,
    Middle,
    Bottom,
}

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

pub struct GlyphPosition {
    pub glyph_id: GlyphId,
    pub x: f32,
    pub y: f32,
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
    fn measure(
        &self,
        config: &TextLayoutConfig,
        font_storage: &mut crate::font_strage::FontStorage,
    ) -> [f32; 2] {
        todo!()
    }

    fn layout(
        &self,
        config: &TextLayoutConfig,
        font_storage: &mut crate::font_strage::FontStorage,
    ) -> TextLayout {
        todo!()
    }
}
