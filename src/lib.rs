#![doc = include_str!("../README.md")]

#![cfg_attr(docsrs, feature(doc_cfg))]

/// Font loading and storage management.
pub mod font_storage;
/// The main entry point for the library, coordinating layout and rendering.
pub mod font_system;
/// Unique identifiers for specific glyphs within a font.
pub mod glyph_id;
/// Rendering backends (CPU, GPU, etc.).
pub mod renderer;
/// Text data structures and layout engine.
pub mod text;

// common re-exports
pub use font_storage::FontStorage;
pub use font_system::FontSystem;
pub use glyph_id::GlyphId;

// re-export dependencies
pub use fontdb;
pub use fontdue;
pub use parking_lot;

#[cfg(feature = "wgpu")]
pub use wgpu;
