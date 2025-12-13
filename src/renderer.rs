pub mod cpu_renderer;
pub mod gpu_renderer;

pub use cpu_renderer::{CpuCacheConfig, CpuRenderer};
pub use gpu_renderer::{AtlasUpdate, GlyphInstance, GpuCacheConfig, GpuRenderer, StandaloneGlyph};

#[cfg(feature = "wgpu")]
pub mod wgpu_renderer;
#[cfg(feature = "wgpu")]
pub use wgpu_renderer::{SimpleRenderPass, WgpuRenderPassController, WgpuRenderer};

// debug uses
#[cfg(all(debug_assertions, feature = "wgpu"))]
pub mod cpu_debug_renderer;
#[cfg(debug_assertions)]
pub mod debug_renderer;
