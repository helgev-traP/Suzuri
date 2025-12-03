/// Collection of text runs that will be laid out together.
///
/// The layout code walks over the stored [`TextElement`] values in order and
/// builds line buffers from them. Keeping the runs grouped here lets the
/// caller reuse the same builder for repeated layout work.
#[derive(Clone, Debug, PartialEq)]
pub struct TextData<T> {
    pub texts: Vec<TextElement<T>>,
}

/// Single run of text that references a font and size.
///
/// A run is processed sequentially during layout so we can merge glyphs that
/// belong to the same font while still respecting wrapping boundaries.
#[derive(Clone, Debug, PartialEq)]
pub struct TextElement<T> {
    pub font_id: fontdb::ID,
    pub font_size: f32,
    pub content: String,
    pub user_data: T,
}

impl<T> Default for TextData<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> TextData<T> {
    /// Creates an empty container that can receive text runs.
    pub fn new() -> Self {
        Self { texts: vec![] }
    }

    /// Adds a new text run to the layout queue.
    ///
    /// Runs are processed in the order they were appended so callers can feed
    /// multiple fonts or styles without copying strings together.
    pub fn append(&mut self, text: TextElement<T>) {
        self.texts.push(text);
    }

    /// Removes all queued text runs so the builder can be reused.
    pub fn clear(&mut self) {
        self.texts.clear();
    }
}
