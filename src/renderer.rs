pub mod cpu_renderer;
pub mod debug_renderer;

pub use cpu_renderer::CpuRenderer;

/// Simple grayscale bitmap used for debugging text layout.
///
/// Pixels are stored in row-major order with the origin at the top-left.
/// Each pixel is a single byte where `0` is background and `255` is white.
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Bitmap {
    fn new(width: usize, height: usize) -> Self {
        let len = width.saturating_mul(height);
        Self {
            width,
            height,
            pixels: vec![0; len],
        }
    }

    /// Adds the given alpha value to the pixel at (x, y).
    /// The result is saturated at 255.
    /// Does nothing if the coordinates are out of bounds.
    #[inline]
    pub fn accumulate(&mut self, x: usize, y: usize, alpha: u8) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = y * self.width + x;
        let existing = self.pixels[idx] as u16;
        let added = alpha as u16;
        self.pixels[idx] = existing.saturating_add(added).min(255) as u8;
    }
}
