use std::path::PathBuf;

use crate::{
    font_storage::FontStorage,
    renderer::{
        CpuRenderer, GpuRenderer,
        cpu_renderer::CpuCacheConfig,
        gpu_renderer::{AtlasUpdate, GlyphInstance, GpuCacheConfig, StandaloneGlyph},
    },
    text::TextLayout,
};

#[cfg(feature = "wgpu")]
use crate::renderer::WgpuRenderer;

pub struct FontSystem {
    font_storage: FontStorage,

    cpu_renderer: Option<Box<CpuRenderer>>,
    gpu_renderer: Option<Box<GpuRenderer>>,
    #[cfg(feature = "wgpu")]
    wgpu_renderer: Option<Box<WgpuRenderer>>,
}

impl Default for FontSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl FontSystem {
    pub fn new() -> Self {
        Self {
            font_storage: FontStorage::new(),
            cpu_renderer: None,
            gpu_renderer: None,
            #[cfg(feature = "wgpu")]
            wgpu_renderer: None,
        }
    }
}

/// font storage initialization
impl FontSystem {
    pub fn load_system_fonts(&mut self) {
        self.font_storage.load_system_fonts();
    }

    pub fn load_font_binary(&mut self, data: impl Into<Vec<u8>>) {
        self.font_storage.load_font_binary(data);
    }

    pub fn load_font_file(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        self.font_storage.load_font_file(path)
    }

    pub fn load_fonts_dir(&mut self, dir: PathBuf) {
        self.font_storage.load_fonts_dir(dir)
    }

    pub fn push_face_info(&mut self, info: fontdb::FaceInfo) {
        self.font_storage.push_face_info(info);
    }

    pub fn remove_face(&mut self, id: fontdb::ID) {
        self.font_storage.remove_face(id);
    }

    pub fn is_empty(&self) -> bool {
        self.font_storage.is_empty()
    }

    pub fn len(&self) -> usize {
        self.font_storage.len()
    }

    pub fn set_serif_family(&mut self, family: impl Into<String>) {
        self.font_storage.set_serif_family(family);
    }

    pub fn set_sans_serif_family(&mut self, family: impl Into<String>) {
        self.font_storage.set_sans_serif_family(family);
    }

    pub fn set_cursive_family(&mut self, family: impl Into<String>) {
        self.font_storage.set_cursive_family(family);
    }

    pub fn set_fantasy_family(&mut self, family: impl Into<String>) {
        self.font_storage.set_fantasy_family(family);
    }

    pub fn set_monospace_family(&mut self, family: impl Into<String>) {
        self.font_storage.set_monospace_family(family);
    }

    pub fn family_name<'a>(&'a self, family: &'a fontdb::Family<'_>) -> &'a str {
        self.font_storage.family_name(family)
    }
}

/// cpu renderer
impl FontSystem {
    pub fn cpu_init(&mut self, configs: &[CpuCacheConfig]) {
        // ensures first drop previous resource to avoid unnecessary memory usage.
        self.cpu_renderer = None;

        self.cpu_renderer = Some(Box::new(CpuRenderer::new(configs)));
    }

    pub fn cpu_cache_clear(&mut self) {
        if let Some(renderer) = &mut self.cpu_renderer {
            renderer.clear_cache();
        } else {
            log::warn!("Cache clear called before cpu renderer initialized.");
        }
    }

    pub fn cpu_render<T>(
        &mut self,
        layout: &TextLayout<T>,
        image_size: [usize; 2],
        f: &mut dyn FnMut([usize; 2], u8, &T),
    ) {
        if let Some(renderer) = &mut self.cpu_renderer {
            renderer.render(layout, image_size, &mut self.font_storage, f);
        } else {
            log::warn!("Render called before cpu renderer initialized.");
        }
    }
}

/// gpu renderer
impl FontSystem {
    pub fn gpu_init(&mut self, configs: &[GpuCacheConfig]) {
        // ensures first drop previous resource to avoid unnecessary memory usage.
        self.gpu_renderer = None;

        self.gpu_renderer = Some(Box::new(GpuRenderer::new(configs)));
    }

    pub fn gpu_cache_clear(&mut self) {
        if let Some(renderer) = &mut self.gpu_renderer {
            renderer.clear_cache();
        } else {
            log::warn!("Cache clear called before gpu renderer initialized.");
        }
    }

    pub fn gpu_render<T: Clone + Copy>(
        &mut self,
        layout: &TextLayout<T>,
        update_atlas: &mut impl FnMut(&[AtlasUpdate]),
        draw_instances: &mut impl FnMut(&[GlyphInstance<T>]),
        draw_standalone: &mut impl FnMut(&StandaloneGlyph<T>),
    ) {
        if let Some(renderer) = &mut self.gpu_renderer {
            renderer.render(
                layout,
                &mut self.font_storage,
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
        &mut self,
        device: &wgpu::Device,
        configs: &[GpuCacheConfig],
        formats: &[wgpu::TextureFormat],
    ) {
        // ensures first drop previous resource and then create new one to avoid unnecessary memory usage.
        self.wgpu_renderer = None;

        self.wgpu_renderer = Some(Box::new(WgpuRenderer::new(device, configs, formats)));
    }

    pub fn wgpu_cache_clear(&mut self) {
        if let Some(renderer) = &mut self.wgpu_renderer {
            renderer.clear_cache();
        } else {
            log::warn!("Cache clear called before wgpu renderer initialized.");
        }
    }

    pub fn wgpu_render<T: Into<[f32; 4]> + Copy>(
        &mut self,
        layout: &TextLayout<T>,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        screen_size: [f32; 2],
    ) {
        if let Some(renderer) = &mut self.wgpu_renderer {
            renderer.render(
                layout,
                &mut self.font_storage,
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
