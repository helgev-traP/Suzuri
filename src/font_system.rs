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
use crate::renderer::WgpuRenderer;

pub struct FontSystem {
    pub font_storage: Mutex<FontStorage>,

    pub cpu_renderer: Mutex<Option<Box<CpuRenderer>>>,
    pub gpu_renderer: Mutex<Option<Box<GpuRenderer>>>,
    #[cfg(feature = "wgpu")]
    pub wgpu_renderer: Mutex<Option<Box<WgpuRenderer>>>,
}

impl Default for FontSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FontSystem {
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
    pub fn load_system_fonts(&self) {
        self.font_storage.lock().load_system_fonts();
    }

    pub fn load_font_binary(&self, data: impl Into<Vec<u8>>) {
        self.font_storage.lock().load_font_binary(data);
    }

    pub fn load_font_file(&self, path: PathBuf) -> Result<(), std::io::Error> {
        self.font_storage.lock().load_font_file(path)
    }

    pub fn load_fonts_dir(&self, dir: PathBuf) {
        self.font_storage.lock().load_fonts_dir(dir)
    }

    pub fn push_face_info(&self, info: fontdb::FaceInfo) {
        self.font_storage.lock().push_face_info(info);
    }

    pub fn remove_face(&self, id: fontdb::ID) {
        self.font_storage.lock().remove_face(id);
    }

    pub fn is_empty(&self) -> bool {
        self.font_storage.lock().is_empty()
    }

    pub fn len(&self) -> usize {
        self.font_storage.lock().len()
    }

    pub fn set_serif_family(&self, family: impl Into<String>) {
        self.font_storage.lock().set_serif_family(family);
    }

    pub fn set_sans_serif_family(&self, family: impl Into<String>) {
        self.font_storage.lock().set_sans_serif_family(family);
    }

    pub fn set_cursive_family(&self, family: impl Into<String>) {
        self.font_storage.lock().set_cursive_family(family);
    }

    pub fn set_fantasy_family(&self, family: impl Into<String>) {
        self.font_storage.lock().set_fantasy_family(family);
    }

    pub fn set_monospace_family(&self, family: impl Into<String>) {
        self.font_storage.lock().set_monospace_family(family);
    }

    pub fn family_name<'a>(&'a self, family: &'a fontdb::Family<'_>) -> String {
        self.font_storage.lock().family_name(family).to_string()
    }
}

/// font querying
impl FontSystem {
    pub fn query(&self, query: &fontdb::Query) -> Option<(fontdb::ID, Arc<fontdue::Font>)> {
        self.font_storage.lock().query(query)
    }

    pub fn font(&self, id: fontdb::ID) -> Option<Arc<fontdue::Font>> {
        self.font_storage.lock().font(id)
    }

    pub fn face(&self, id: fontdb::ID) -> Option<fontdb::FaceInfo> {
        self.font_storage.lock().face(id).cloned()
    }

    pub fn face_source(&self, id: fontdb::ID) -> Option<(fontdb::Source, u32)> {
        self.font_storage.lock().face_source(id)
    }
}

/// text layout
impl FontSystem {
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
    pub fn cpu_init(&self, configs: &[CpuCacheConfig]) {
        // ensures first drop previous resource to avoid unnecessary memory usage.
        *self.cpu_renderer.lock() = None;

        *self.cpu_renderer.lock() = Some(Box::new(CpuRenderer::new(configs)));
    }

    pub fn cpu_cache_clear(&self) {
        if let Some(renderer) = &mut *self.cpu_renderer.lock() {
            renderer.clear_cache();
        } else {
            log::warn!("Cache clear called before cpu renderer initialized.");
        }
    }

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
    pub fn gpu_init(&self, configs: &[GpuCacheConfig]) {
        // ensures first drop previous resource to avoid unnecessary memory usage.
        *self.gpu_renderer.lock() = None;

        *self.gpu_renderer.lock() = Some(Box::new(GpuRenderer::new(configs)));
    }

    pub fn gpu_cache_clear(&self) {
        if let Some(renderer) = &mut *self.gpu_renderer.lock() {
            renderer.clear_cache();
        } else {
            log::warn!("Cache clear called before gpu renderer initialized.");
        }
    }

    pub fn gpu_render<T: Clone + Copy>(
        &self,
        layout: &TextLayout<T>,
        update_atlas: &mut impl FnMut(&[AtlasUpdate]),
        draw_instances: &mut impl FnMut(&[GlyphInstance<T>]),
        draw_standalone: &mut impl FnMut(&StandaloneGlyph<T>),
    ) {
        if let Some(renderer) = &mut *self.gpu_renderer.lock() {
            renderer.render(
                layout,
                &mut self.font_storage.lock(),
                update_atlas,
                draw_instances,
                draw_standalone,
            );
        } else {
            log::warn!("Render called before gpu renderer initialized.");
        }
    }
}

/// wgpu renderer
#[cfg(feature = "wgpu")]
impl FontSystem {
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

    pub fn wgpu_cache_clear(&self) {
        if let Some(renderer) = &mut *self.wgpu_renderer.lock() {
            renderer.clear_cache();
        } else {
            log::warn!("Cache clear called before wgpu renderer initialized.");
        }
    }

    pub fn wgpu_render<T: Into<[f32; 4]> + Copy>(
        &self,
        layout: &TextLayout<T>,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        screen_size: [f32; 2],
    ) {
        if let Some(renderer) = &mut *self.wgpu_renderer.lock() {
            renderer.render(
                layout,
                &mut self.font_storage.lock(),
                device,
                encoder,
                view,
                screen_size,
            );
        } else {
            log::warn!("Render called before wgpu renderer initialized.");
        }
    }
}
