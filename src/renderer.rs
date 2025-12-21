/// CPU software renderer.
pub mod cpu_renderer;
/// Hardware-agnostic GPU renderer.
pub mod gpu_renderer;

pub use cpu_renderer::{CpuCacheConfig, CpuRenderer};
pub use gpu_renderer::{AtlasUpdate, GlyphInstance, GpuCacheConfig, GpuRenderer, StandaloneGlyph};

#[cfg(feature = "wgpu")]
pub mod wgpu_renderer;
#[cfg(feature = "wgpu")]
pub use wgpu_renderer::{SimpleRenderPass, WgpuRenderPassController, WgpuRenderer};

// debug uses
/// CPU-based debugging renderer.
#[cfg(all(debug_assertions, feature = "wgpu"))]
#[doc(hidden)]
pub mod cpu_debug_renderer;
/// Simple bitmap debug renderer.
#[cfg(debug_assertions)]
#[doc(hidden)]
pub mod debug_renderer;
