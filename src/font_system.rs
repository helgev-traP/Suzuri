use std::{path::PathBuf, sync::Arc};

use parking_lot::Mutex;

use crate::{
    font_storage::FontStorage,
    renderer::{
        CpuRenderer, GpuRenderer,
        cpu_renderer::CpuCacheConfig,
        gpu_renderer::{AtlasUpdate, GlyphInstance, GpuCacheConfig, StandaloneGlyph},
    },
    text::{TextData, TextLayout, TextLayoutConfig},
};

#[cfg(feature = "wgpu")]
use crate::renderer::{WgpuRenderPassController, WgpuRenderer};

/// High-level entry point for the text rendering system.
///
/// This struct coordinates `FontStorage`, `TextLayout`, and various renderers (CPU, GPU, and WGPU if "wgpu" feature is enabled).
/// It provides a unified interface for loading fonts, laying out text, and rendering it.
///
/// Use `Mutex` to allow shared mutable access, which is common in UI frameworks.
///
/// The fields are public to allow direct access to the underlying storage and renderers when necessary
/// (e.g. for performance reasons or zero-allocation access).
pub struct FontSystem {
    /// The underlying font storage.
    pub font_storage: Mutex<FontStorage>,

    /// The CPU renderer instance (optional).
    pub cpu_renderer: Mutex<Option<Box<CpuRenderer>>>,
    /// The generic GPU renderer instance (optional).
    pub gpu_renderer: Mutex<Option<Box<GpuRenderer>>>,
    #[cfg(feature = "wgpu")]
    /// The wgpu renderer instance (optional).
    pub wgpu_renderer: Mutex<Option<Box<WgpuRenderer>>>,
}

impl Default for FontSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FontSystem {
    /// Creates a new font system with empty renderers and default storage.
    pub fn new() -> Self {
        Self {
            font_storage: Mutex::new(FontStorage::new()),
            cpu_renderer: Mutex::new(None),
            gpu_renderer: Mutex::new(None),
            #[cfg(feature = "wgpu")]
            wgpu_renderer: Mutex::new(None),
        }
    }
}

/// font storage initialization
impl FontSystem {
    /// Loads the system fonts into the storage.
    pub fn load_system_fonts(&self) {
        self.font_storage.lock().load_system_fonts();
    }

    /// Loads a font from binary data.
    pub fn load_font_binary(&self, data: impl Into<Vec<u8>>) {
        self.font_storage.lock().load_font_binary(data);
    }

    /// Loads a font from a file path.
    pub fn load_font_file(&self, path: PathBuf) -> Result<(), std::io::Error> {
        self.font_storage.lock().load_font_file(path)
    }

    /// Loads all fonts from a directory.
    pub fn load_fonts_dir(&self, dir: PathBuf) {
        self.font_storage.lock().load_fonts_dir(dir)
    }

    /// Manually adds a face info.
    pub fn push_face_info(&self, info: fontdb::FaceInfo) {
        self.font_storage.lock().push_face_info(info);
    }

    /// Removes a face by ID.
    pub fn remove_face(&self, id: fontdb::ID) {
        self.font_storage.lock().remove_face(id);
    }

    /// Checks if the storage is empty.
    pub fn is_empty(&self) -> bool {
        self.font_storage.lock().is_empty()
    }

    /// Returns the number of loaded faces.
    pub fn len(&self) -> usize {
        self.font_storage.lock().len()
    }

    /// Sets the family name for the "serif" generic family.
    pub fn set_serif_family(&self, family: impl Into<String>) {
        self.font_storage.lock().set_serif_family(family);
    }

    /// Sets the family name for the "sans-serif" generic family.
    pub fn set_sans_serif_family(&self, family: impl Into<String>) {
        self.font_storage.lock().set_sans_serif_family(family);
    }

    /// Sets the family name for the "cursive" generic family.
    pub fn set_cursive_family(&self, family: impl Into<String>) {
        self.font_storage.lock().set_cursive_family(family);
    }

    /// Sets the family name for the "fantasy" generic family.
    pub fn set_fantasy_family(&self, family: impl Into<String>) {
        self.font_storage.lock().set_fantasy_family(family);
    }

    /// Sets the family name for the "monospace" generic family.
    pub fn set_monospace_family(&self, family: impl Into<String>) {
        self.font_storage.lock().set_monospace_family(family);
    }

    /// Returns the name of a family.
    ///
    /// # Performance
    /// This method allocates a new `String` to avoid holding a lock on the storage.
    /// If you need zero-allocation access, lock `font_storage` directly.
    pub fn family_name<'a>(&'a self, family: &'a fontdb::Family<'_>) -> String {
        self.font_storage.lock().family_name(family).to_string()
    }
}

/// font querying
impl FontSystem {
    /// Queries for a font matching the description.
    pub fn query(&self, query: &fontdb::Query) -> Option<(fontdb::ID, Arc<fontdue::Font>)> {
        self.font_storage.lock().query(query)
    }

    /// Retrieves a loaded font by ID.
    pub fn font(&self, id: fontdb::ID) -> Option<Arc<fontdue::Font>> {
        self.font_storage.lock().font(id)
    }

    /// Returns a vec over all available faces.
    ///
    /// # Performance
    /// This method clones all face info to avoid holding a lock on the storage.
    /// If you need to iterate without allocation, lock `font_storage` directly.
    pub fn faces(&self) -> Vec<fontdb::FaceInfo> {
        self.font_storage.lock().faces().cloned().collect()
    }

    /// Returns face info for an ID.
    ///
    /// # Performance
    /// This method clones the face info to avoid holding a lock on the storage.
    /// If you need reference access, lock `font_storage` directly.
    pub fn face(&self, id: fontdb::ID) -> Option<fontdb::FaceInfo> {
        self.font_storage.lock().face(id).cloned()
    }

    /// Returns the source of a face.
    pub fn face_source(&self, id: fontdb::ID) -> Option<(fontdb::Source, u32)> {
        self.font_storage.lock().face_source(id)
    }
}

/// text layout
impl FontSystem {
    /// Performs text layout using the fonts in this system.
    pub fn layout_text<T: Clone>(
        &self,
        text: &TextData<T>,
        config: &TextLayoutConfig,
    ) -> TextLayout<T> {
        let mut font_storage = self.font_storage.lock();
        text.layout(config, &mut font_storage)
    }
}

/// cpu renderer
impl FontSystem {
    /// Initializes the CPU renderer with the given cache configuration.
    ///
    /// This will replace any existing CPU renderer.
    pub fn cpu_init(&self, configs: &[CpuCacheConfig]) {
        // ensures first drop previous resource to avoid unnecessary memory usage.
        *self.cpu_renderer.lock() = None;

        *self.cpu_renderer.lock() = Some(Box::new(CpuRenderer::new(configs)));
    }

    /// Initializes the CPU renderer with the given cache configuration if it is not already initialized.
    pub fn cpu_ensure_init(&self, configs: &[CpuCacheConfig]) {
        if self.cpu_renderer.lock().is_none() {
            self.cpu_init(configs);
        }
    }

    /// Clears the CPU renderer's cache.
    pub fn cpu_cache_clear(&self) {
        if let Some(renderer) = &mut *self.cpu_renderer.lock() {
            renderer.clear_cache();
        } else {
            log::warn!("Cache clear called before cpu renderer initialized.");
        }
    }

    /// Renders text using the CPU renderer.
    ///
    /// The callback `f` is called for each pixel.
    pub fn cpu_render<T>(
        &self,
        layout: &TextLayout<T>,
        image_size: [usize; 2],
        f: &mut dyn FnMut([usize; 2], u8, &T),
    ) {
        if let Some(renderer) = &mut *self.cpu_renderer.lock() {
            renderer.render(layout, image_size, &mut self.font_storage.lock(), f);
        } else {
            log::warn!("Render called before cpu renderer initialized.");
        }
    }
}

/// gpu renderer
impl FontSystem {
    /// Initializes the generic GPU renderer with the given cache configuration.
    ///
    /// This will replace any existing GPU renderer.
    pub fn gpu_init(&self, configs: &[GpuCacheConfig]) {
        // ensures first drop previous resource to avoid unnecessary memory usage.
        *self.gpu_renderer.lock() = None;

        *self.gpu_renderer.lock() = Some(Box::new(GpuRenderer::new(configs)));
    }

    /// Initializes the generic GPU renderer with the given cache configuration if it is not already initialized.
    pub fn gpu_ensure_init(&self, configs: &[GpuCacheConfig]) {
        if self.gpu_renderer.lock().is_none() {
            self.gpu_init(configs);
        }
    }

    /// Clears the generic GPU renderer's cache.
    pub fn gpu_cache_clear(&self) {
        if let Some(renderer) = &mut *self.gpu_renderer.lock() {
            renderer.clear_cache();
        } else {
            log::warn!("Cache clear called before gpu renderer initialized.");
        }
    }

    /// Renders text using the generic GPU renderer.
    ///
    /// This requires providing callbacks to handle atlas updates and drawing.
    /// This method is for infallible callbacks. Use `try_gpu_render` for fallible callbacks.
    pub fn gpu_render<T: Clone + Copy>(
        &self,
        layout: &TextLayout<T>,
        update_atlas: impl FnMut(&[AtlasUpdate]),
        draw_instances: impl FnMut(&[GlyphInstance<T>]),
        draw_standalone: impl FnMut(&StandaloneGlyph<T>),
    ) {
        if let Some(renderer) = &mut *self.gpu_renderer.lock() {
            renderer.render(
                layout,
                &mut self.font_storage.lock(),
                update_atlas,
                draw_instances,
                draw_standalone,
            )
        } else {
            log::warn!("Render called before gpu renderer initialized.");
        }
    }

    /// Renders text using the generic GPU renderer.
    ///
    /// This requires providing callbacks to handle atlas updates and drawing.
    /// This method allows callbacks to return errors, which will be propagated.
    pub fn try_gpu_render<T: Clone + Copy, E>(
        &self,
        layout: &TextLayout<T>,
        update_atlas: &mut impl FnMut(&[AtlasUpdate]) -> Result<(), E>,
        draw_instances: &mut impl FnMut(&[GlyphInstance<T>]) -> Result<(), E>,
        draw_standalone: &mut impl FnMut(&StandaloneGlyph<T>) -> Result<(), E>,
    ) -> Result<(), E> {
        if let Some(renderer) = &mut *self.gpu_renderer.lock() {
            renderer.try_render(
                layout,
                &mut self.font_storage.lock(),
                update_atlas,
                draw_instances,
                draw_standalone,
            )
        } else {
            log::warn!("Render called before gpu renderer initialized.");
            Ok(())
        }
    }
}

/// wgpu renderer
#[cfg(feature = "wgpu")]
impl FontSystem {
    /// Initializes the WGPU renderer.
    ///
    /// `configs` specifies the atlas configuration.
    /// `formats` specifies the texture formats that will be used for rendering, allowing pipeline pre-compilation.
    pub fn wgpu_init(
        &self,
        device: &wgpu::Device,
        configs: &[GpuCacheConfig],
        formats: &[wgpu::TextureFormat],
    ) {
        // ensures first drop previous resource and then create new one to avoid unnecessary memory usage.
        *self.wgpu_renderer.lock() = None;

        *self.wgpu_renderer.lock() = Some(Box::new(WgpuRenderer::new(device, configs, formats)));
    }

    /// Initializes the WGPU renderer with the given cache configuration if it is not already initialized.
    pub fn wgpu_ensure_init(
        &self,
        device: &wgpu::Device,
        configs: &[GpuCacheConfig],
        formats: &[wgpu::TextureFormat],
    ) {
        if self.wgpu_renderer.lock().is_none() {
            self.wgpu_init(device, configs, formats);
        }
    }

    /// Clears the WGPU renderer's cache.
    pub fn wgpu_cache_clear(&self) {
        if let Some(renderer) = &mut *self.wgpu_renderer.lock() {
            renderer.clear_cache();
        } else {
            log::warn!("Cache clear called before wgpu renderer initialized.");
        }
    }

    /// Renders text using the WGPU renderer.
    pub fn wgpu_render<T: Into<[f32; 4]> + Copy>(
        &self,
        text_layout: &TextLayout<T>,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        if let Some(renderer) = &mut *self.wgpu_renderer.lock() {
            renderer.render(
                text_layout,
                &mut self.font_storage.lock(),
                device,
                encoder,
                view,
            );
        } else {
            log::warn!("Render called before wgpu renderer initialized.");
        }
    }

    pub fn wgpu_render_to<T: Into<[f32; 4]> + Copy, E>(
        &self,
        text_layout: &TextLayout<T>,
        device: &wgpu::Device,
        controller: &mut impl WgpuRenderPassController<E>,
    ) -> Result<(), E> {
        if let Some(renderer) = &mut *self.wgpu_renderer.lock() {
            renderer.render_to(
                text_layout,
                &mut self.font_storage.lock(),
                device,
                controller,
            )?;

            Ok(())
        } else {
            log::warn!("Render called before wgpu renderer initialized.");
            Ok(())
        }
    }
}
