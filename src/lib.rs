//! # Suzuri
//!
//! Suzuri is a text rendering library written in Rust.
//! Use this crate to layout and render text with consistent results.
//!
//! This crate prioritizes consistency and reproducibility within a two-pass layout data flow.
//! It ensures that layout results remain stable even when constraints change, provided there is enough
//! whitespace (e.g., preventing unstable line breaks when layout width is reduced).
//!
//! ## Overview
//!
//! The library is composed of the following main functionalities:
//!
//! *   **[`font_storage::FontStorage`]**: Manages font loading and caching. It handles system fonts and custom font data.
//! *   **Text Representation**: Defined by [`text::TextData`] and [`text::TextElement`]. These structures hold the content and styling information (font, size, color, etc.) for text.
//! *   **[`text::TextLayout`]**: The engine that calculates glyph positions, handling wrapping, alignment, and other layout properties based on the configuration.
//! *   **Renderers**:
//!     *   **[`renderer::CpuRenderer`]**: Renders text into a pixel buffer on the CPU.
//!     *   **[`renderer::GpuRenderer`]**: A graphics-API-independent text renderer. It manages texture atlases and glyph quads, allowing implementation on any graphics backend (e.g., OpenGL, Vulkan, DirectX).
//!     *   **[`renderer::WgpuRenderer`]**: A concrete implementation built on top of `GpuRenderer` using the WGPU graphics API.
//!
//! The [`FontSystem`] acts as the central hub, coordinating these components to provide a unified API.
//!
//! ## How to Read the Documentation
//!
//! To get started with Suzuri, we recommend reading the documentation in the following order:
//!
//! 1.  **[`FontSystem`]**: The entry point of the library. Learn how to initialize the system.
//! 2.  **[`fontdb`]**: Understanding `fontdb` is essential for querying fonts (by family, weight, style) to obtain the Font ID needed for text elements.
//! 3.  **[`text::TextData`] & [`text::TextElement`]**: Learn how to create text content with styles and custom data.
//! 4.  **[`text::TextLayout`]**: Understand how the text is measured and arranged.
//! 5.  **Renderers**: Finally, explore the specific renderer you intend to use (e.g., in [`renderer`] module) for integration details.
//!
//! ## Usage
//!
//! Here is a complete example of rendering text to a byte buffer (grayscale image) using the CPU renderer.
//!
//! ```rust,no_run
//! use suzuri::{
//!     FontSystem, fontdb,
//!     text::{TextData, TextElement, TextLayoutConfig},
//!     renderer::CpuCacheConfig
//! };
//! use std::num::NonZeroUsize;
//!
//! // 1. Create a FontSystem
//! let font_system = FontSystem::new();
//! font_system.load_system_fonts();
//!
//! // 2. Prepare text data
//! // In this example, 'user_data' is a u8 representing brightness (0-255).
//! let mut data = TextData::<u8>::new();
//!
//! // Query a font (e.g., SansSerif)
//! let query = fontdb::Query {
//!     families: &[fontdb::Family::SansSerif],
//!     weight: fontdb::Weight::NORMAL,
//!     stretch: fontdb::Stretch::Normal,
//!     style: fontdb::Style::Normal,
//! };
//! let font_id = font_system.query(&query).map(|(id, _font)| id);
//!
//! if let Some(id) = font_id {
//!     data.append(TextElement {
//!         content: "Hello world".to_string(),
//!         font_id: id,
//!         font_size: 24.0,
//!         user_data: 255u8, // White text
//!     });
//! }
//!
//! // 3. Layout the text
//! let layout = font_system.layout_text(&data, &TextLayoutConfig::default());
//!
//! // 4. Initialize CPU Renderer
//! // Configurations for glyph cache (small and large glyphs)
//! let cache_configs = [
//!     CpuCacheConfig {
//!         block_size: NonZeroUsize::new(32 * 32).unwrap(), // For small glyphs
//!         capacity: NonZeroUsize::new(1024).unwrap(),
//!     },
//!     CpuCacheConfig {
//!         block_size: NonZeroUsize::new(128 * 128).unwrap(), // For large glyphs
//!         capacity: NonZeroUsize::new(128).unwrap(),
//!     },
//! ];
//! font_system.cpu_init(&cache_configs);
//!
//! // 5. Render to a buffer (e.g., a grayscale image)
//! let width = 800;
//! let height = 600;
//! let mut buffer = vec![0u8; width * height]; // Black background
//!
//! font_system.cpu_render(
//!     &layout,
//!     [width, height],
//!     &mut |pos, alpha, color| {
//!         // 'pos' is [x, y] in pixels.
//!         // 'alpha' is the coverage of the glyph (0-255).
//!         // 'color' is the user_data we set earlier (255).
//!
//!         let idx = pos[1] * width + pos[0];
//!         
//!         // specific blending logic (e.g. additive blending)
//!         let val = (alpha as u16 * *color as u16 / 255) as u8;
//!         buffer[idx] = buffer[idx].saturating_add(val);
//!     }
//! );
//! ```
//!
//! ## Features
//!
//! *   **Flexible Backend**: Supports both CPU-based rendering and GPU acceleration (via WGPU).
//! *   **Robust Layout**: Handles text wrapping, alignment, and multi-font shaping with predictable results.
//! *   **Font Management**: Easy loading of system fonts and custom font files via `fontdb`.
//! *   **Thread Safety**: Designed with internal locking for safe concurrent use.

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
