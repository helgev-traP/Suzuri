/// Defines the input data structures for text layout.
pub mod data;
/// The core text layout engine and configuration.
pub mod layout;

pub use data::{TextData, TextElement};
pub use layout::{
    GlyphPosition, HorizontalAlign, TextLayout, TextLayoutConfig, TextLayoutLine, VerticalAlign,
    WrapStyle,
};
