use super::gpu_renderer::{
    AtlasUpdate, GlyphInstance, GpuCacheConfig, GpuRenderer, StandaloneGlyph,
};
use crate::font_storage::FontStorage;
use crate::text::TextLayout;
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

/// Initial capacity for the instance buffer.
/// Chosen to balance memory usage and typical text rendering workloads
/// (average paragraph with ~250-500 glyphs, with headroom for multiple draw calls).
const INITIAL_INSTANCE_CAPACITY: usize = 1024;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct InstanceData {
    screen_rect: [f32; 4], // x, y, w, h
    uv_rect: [f32; 4],     // u, v, w, h
    color: [f32; 4],
    layer: u32,
    _padding: [u32; 3],
}

impl InstanceData {
    /// Returns the vertex buffer layout for instance data.
    ///
    /// This layout is shared between the main atlas pipeline and the standalone pipeline.
    const ATTRIBUTES: &'static [wgpu::VertexAttribute] = &[
        // screen_rect
        wgpu::VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: wgpu::VertexFormat::Float32x4,
        },
        // uv_rect
        wgpu::VertexAttribute {
            offset: 16,
            shader_location: 1,
            format: wgpu::VertexFormat::Float32x4,
        },
        // color
        wgpu::VertexAttribute {
            offset: 32,
            shader_location: 2,
            format: wgpu::VertexFormat::Float32x4,
        },
        // layer
        wgpu::VertexAttribute {
            offset: 48,
            shader_location: 3,
            format: wgpu::VertexFormat::Uint32,
        },
    ];

    fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: Self::ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Globals {
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

/// A text renderer using `wgpu` for hardware-accelerated rendering.
///
/// ## Overview
///
/// `WgpuRenderer` is a high-level wrapper around [`GpuRenderer`] tailored for the WGPU ecosystem.
/// It handles all GPU resource management, including:
///
/// *   **Texture Atlases**: Creating and updating textures for caching glyphs.
/// *   **Pipelines**: Managing render pipelines for different texture formats.
/// *   **Buffers**: Handling vertex/index/uniform buffers.
/// *   **Shaders**: Providing built-in WGSL shaders for text rendering.
///
/// It supports **Premultiplied Alpha** blending for correct color composition.
///
/// ## Integration
///
/// This component can be used in two ways:
/// -   **Through [`crate::FontSystem`]**: Provides a high-level API where `FontSystem` manages the renderer instance.
/// -   **Standalone**: You can instantiate and use this renderer directly. This offers more granular control over resource management and rendering.
///
/// ## Usage
///
/// ```rust,no_run
/// use suzuri::{
///     FontSystem, fontdb,
///     renderer::GpuCacheConfig,
///     text::{TextData, TextElement, TextLayoutConfig}
/// };
/// use std::num::NonZeroUsize;
///
/// // Assume standard wgpu setup (device, queue, etc.)
/// # async fn example() {
/// # let (device, queue): (wgpu::Device, wgpu::Queue) = todo!();
/// # let texture_format = wgpu::TextureFormat::Bgra8Unorm;
/// # let view: wgpu::TextureView = todo!();
/// # let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
///
/// let font_system = FontSystem::new();
/// font_system.load_system_fonts();
///
/// // 1. Initialize Renderer
/// let cache_configs = [
///     GpuCacheConfig {
///         texture_size: NonZeroUsize::new(1024).unwrap(),
///         tile_size: NonZeroUsize::new(32).unwrap(), // one side length
///         tiles_per_axis: NonZeroUsize::new(32).unwrap(),
///     },
/// ];
/// // Pre-compile pipeline for the target format
/// font_system.wgpu_init(&device, &cache_configs, &[texture_format]);
///
/// // 2. Layout Text
/// let mut data: TextData<[f32; 4]> = TextData::new();
/// // ... (append text elements) ...
/// let layout = font_system.layout_text(&data, &TextLayoutConfig::default());
///
/// // 3. Render
/// font_system.wgpu_render(
///     &layout,
///     &device,
///     &mut encoder,
///     &view
/// );
/// # }
/// ```
///
/// # Color Handling
///
/// The renderer expects user data to be convertible to `[f32; 4]` representing
/// **Premultiplied Alpha** color.
///
/// - **Input Format**: `[r, g, b, a]` where components are premultiplied by alpha.
///   - Example: 50% transparent white should be `[0.5, 0.5, 0.5, 0.5]`, NOT `[1.0, 1.0, 1.0, 0.5]`.
/// - **Compositing**: The renderer performs standard usage of the alpha masking from the font atlas.
///   It applies the mask to the input color. The pipeline is configured with `PREMULTIPLIED_ALPHA_BLENDING`.
///
/// # Performance Optimizations
///
/// ## Pipeline Caching
/// The renderer creates render pipelines lazily based on the `TextureFormat` of the render target.
/// This means the first `render` call for a new format might incur a small delay.
///
/// To avoid runtime hitches, you can pre-warm the cache by supplying expected formats
/// during initialization:
/// ```rust,no_run
/// # use suzuri::{FontSystem, renderer::GpuCacheConfig};
/// # use std::num::NonZeroUsize;
/// # let (device, queue): (wgpu::Device, wgpu::Queue) = todo!();
/// # let cache_configs = [];
/// let font_system = FontSystem::new();
/// font_system.wgpu_init(
///     &device,
///     &cache_configs,
///     &[wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm] // Pre-compile these
/// );
/// ```
///
/// # Important Notes
/// - **Atlas Management**: The renderer manages an internal texture atlas array.
///   It automatically handles updates and uploads. Ensure `configs` passed to `new`
///   are sufficient for your text usage preventing frequent cache trashing (fallback strategy handles overflow but can be slower).
/// - **Command Encoder**: The `render` method takes a mutable `CommandEncoder`. It will record
///   copy commands (for atlas/uniform updates) and a render pass.
/// - **Thread Safety**: `WgpuRenderer` employs internal mutability (`RefCell`) for resource
///   management, so it is **not** `Sync`. Even though `wgpu` resources are thread-safe,
///   this renderer is designed to be used from a single thread (usually the main render thread).
pub struct WgpuRenderer {
    pub gpu_renderer: GpuRenderer,
    resources: WgpuResources,
}

/// Resources used by the renderer, including pipelines, buffers, and textures.
///
/// This struct uses `RefCell` for internal mutability, allowing the `render` method
/// to update resources (like buffers and caches) while retaining an immutable interface
/// where possible, or satisfying the borrowing rules of helper methods.
struct WgpuResources {
    /// Cache of pipelines for different texture formats (e.g., specific swapchain formats).
    pipelines: std::cell::RefCell<HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>>,
    /// Cache of pipelines for standalone large glyphs.
    standalone_pipelines: std::cell::RefCell<HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>>,

    pipeline_layout: wgpu::PipelineLayout,
    standalone_pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    standalone_shader: wgpu::ShaderModule,

    /// The texture atlas array used for caching small glyphs.
    atlas_texture: wgpu::Texture,
    sampler: wgpu::Sampler,

    /// Shared instance buffer for drawing glyph quads. Resizes automatically.
    instance_buffer: std::cell::RefCell<wgpu::Buffer>,

    _bind_group_layout: wgpu::BindGroupLayout,
    standalone_bind_group_layout: wgpu::BindGroupLayout,

    /// Uniform buffer for global data (screen size, etc.).
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,

    /// Resources for drawing a single large glyph that doesn't fit in the atlas.
    standalone_resources: std::cell::RefCell<Option<StandaloneResources>>,

    /// **Staging Vector for Instance Data**
    /// Reused across frames to avoid repeated allocations (`Vec::new()`) when building instance data.
    instance_data_staging: std::cell::RefCell<Vec<InstanceData>>,

    /// **Staging Vector for Pixel Padding**
    /// Reused across frames to avoid allocations when padding texture data to 256-byte alignment.
    pixel_staging: std::cell::RefCell<Vec<u8>>,
}

/// Resources required for rendering a standalone large glyph.
struct StandaloneResources {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    /// Current size of the texture. Used to determine if re-creation is needed.
    size: wgpu::Extent3d,
}

const SHADER: &str = include_str!("wgpu_renderer/wgpu_renderer_shader.wgsl");

const STANDALONE_SHADER: &str = include_str!("wgpu_renderer/wgpu_renderer_standalone.wgsl");

impl WgpuRenderer {
    /// Requires at least one `GpuCacheConfig`.
    ///
    /// # Panics
    ///
    /// Panics if `configs` is empty.
    pub fn new(
        device: &wgpu::Device,
        configs: &[GpuCacheConfig],
        formats: &[wgpu::TextureFormat],
    ) -> Self {
        if configs.is_empty() {
            log::error!("At least one GPU cache config is required");
            panic!("At least one GPU cache config is required");
        }

        let gpu_renderer = GpuRenderer::new(configs);

        // Calculate max dimensions and layers
        let max_width = configs
            .iter()
            .map(|c| c.texture_size.get())
            .max()
            .expect("Checked above") as u32;
        let max_height = configs
            .iter()
            .map(|c| c.texture_size.get())
            .max()
            .expect("Checked above") as u32;
        let layers = configs.len() as u32;

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Atlas Array"),
            size: wgpu::Extent3d {
                width: max_width,
                height: max_height,
                depth_or_array_layers: layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("WgpuRenderer Bind Group Layout"),
            entries: &[
                // Globals
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Texture Array
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        // Standalone layout (Texture 2D instead of Array)
        let standalone_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("WgpuRenderer Standalone Bind Group Layout"),
                entries: &[
                    // Globals
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Texture 2D
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("WgpuRenderer Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let standalone_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("WgpuRenderer Standalone Pipeline Layout"),
                bind_group_layouts: &[&standalone_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("WgpuRenderer Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let standalone_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("WgpuRenderer Standalone Shader"),
            source: wgpu::ShaderSource::Wgsl(STANDALONE_SHADER.into()),
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: (INITIAL_INSTANCE_CAPACITY * std::mem::size_of::<InstanceData>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Globals Buffer"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Globals Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
            ],
        });

        let resources = WgpuResources {
            pipelines: std::cell::RefCell::new(HashMap::new()),
            standalone_pipelines: std::cell::RefCell::new(HashMap::new()),
            pipeline_layout,
            standalone_pipeline_layout,
            shader,
            standalone_shader,
            atlas_texture,
            sampler,
            instance_buffer: std::cell::RefCell::new(instance_buffer),
            _bind_group_layout: bind_group_layout,
            standalone_bind_group_layout,
            globals_buffer,
            globals_bind_group,
            standalone_resources: std::cell::RefCell::new(None),
            instance_data_staging: std::cell::RefCell::new(Vec::new()),
            pixel_staging: std::cell::RefCell::new(Vec::new()),
        };

        for &format in formats {
            resources.get_pipeline(device, format);
            resources.get_standalone_pipeline(device, format);
        }

        Self {
            gpu_renderer,
            resources,
        }
    }

    /// Clears the renderer's cache, freeing GPU memory.
    pub fn clear_cache(&mut self) {
        self.gpu_renderer.clear_cache();
    }
}

/// Abstraction for managing a render pass.
///
/// This trait allows `WgpuRenderer` to work with different contexts, such as a direct
/// `RenderPass` creation or a deferred command recording mechanism.
/// It primarily exists to break the borrow checker deadlock where `encoder` (mutable)
/// and `texture_view` (immutable) might be tied together inconveniently.
pub trait WgpuRenderPassController<E = ()> {
    /// Returns the mutable command encoder to record copy commands.
    fn encoder(&mut self) -> Result<&mut wgpu::CommandEncoder, E>;

    /// Creates a new `RenderPass`.
    /// Note: The lifetime is tied to the controller to enforce correct usage scope.
    fn create_pass(&mut self) -> Result<wgpu::RenderPass<'_>, E>;

    /// Returns the target texture format for pipeline selection.
    fn format(&self) -> Result<wgpu::TextureFormat, E>;

    /// Returns the target screen size in pixels.
    fn target_size(&self) -> Result<[f32; 2], E>;
}

impl<T: WgpuRenderPassController<E> + ?Sized, E> WgpuRenderPassController<E> for &mut T {
    fn encoder(&mut self) -> Result<&mut wgpu::CommandEncoder, E> {
        (**self).encoder()
    }

    fn create_pass(&mut self) -> Result<wgpu::RenderPass<'_>, E> {
        (**self).create_pass()
    }

    fn format(&self) -> Result<wgpu::TextureFormat, E> {
        (**self).format()
    }

    fn target_size(&self) -> Result<[f32; 2], E> {
        (**self).target_size()
    }
}

/// A simple implementation of `WgpuRenderPassController` that renders to a given view.
///
/// It clears the screen on the first draw call and loads on subsequent calls.
/// This matches the typical behavior for rendering text overlay.
pub struct SimpleRenderPass<'a> {
    encoder: &'a mut wgpu::CommandEncoder,
    view: &'a wgpu::TextureView,
    first_call: bool,
    clear_color: wgpu::Color,
}

impl<'a> SimpleRenderPass<'a> {
    /// Creates a new `SimpleRenderPass`.
    ///
    /// By default, it clears to Black (0,0,0,1).
    pub fn new(encoder: &'a mut wgpu::CommandEncoder, view: &'a wgpu::TextureView) -> Self {
        Self {
            encoder,
            view,
            first_call: true,
            clear_color: wgpu::Color::BLACK,
        }
    }

    /// Sets the clear color used on the first pass.
    pub fn with_clear_color(mut self, color: wgpu::Color) -> Self {
        self.clear_color = color;
        self
    }
}

impl<'a> WgpuRenderPassController<()> for SimpleRenderPass<'a> {
    fn encoder(&mut self) -> Result<&mut wgpu::CommandEncoder, ()> {
        Ok(self.encoder)
    }

    fn create_pass(&mut self) -> Result<wgpu::RenderPass<'_>, ()> {
        let load = if self.first_call {
            self.first_call = false;
            wgpu::LoadOp::Clear(self.clear_color)
        } else {
            wgpu::LoadOp::Load
        };

        Ok(self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("WgpuRenderer Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: self.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        }))
    }

    fn format(&self) -> Result<wgpu::TextureFormat, ()> {
        Ok(self.view.texture().format())
    }

    fn target_size(&self) -> Result<[f32; 2], ()> {
        let size = self.view.texture().size();
        Ok([size.width as f32, size.height as f32])
    }
}

impl WgpuRenderer {
    pub fn render<T: Into<[f32; 4]> + Copy>(
        &mut self,
        text_layout: &TextLayout<T>,
        font_storage: &mut FontStorage,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        let mut ctx = SimpleRenderPass::new(encoder, view);

        self.render_to(text_layout, font_storage, device, &mut ctx)
            .expect("`SimpleRenderPass` never fails.")
    }

    /// Renders the layout using a custom render pass controller.
    ///
    /// This method allows for more flexible rendering scenarios where the render pass
    /// creation or management is handled externally via the `WgpuRenderPassController` trait.
    pub fn render_to<T: Into<[f32; 4]> + Copy, E>(
        &mut self,
        text_layout: &TextLayout<T>,
        font_storage: &mut FontStorage,
        device: &wgpu::Device,
        controller: &mut impl WgpuRenderPassController<E>,
    ) -> Result<(), E> {
        // Reset offset at the beginning of the frame
        let current_offset = std::cell::Cell::new(0);

        // Update globals
        let globals = Globals {
            screen_size: controller.target_size()?,
            _padding: [0.0; 2],
        };
        let globals_staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Globals Staging Buffer"),
            contents: bytemuck::bytes_of(&globals),
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        controller.encoder()?.copy_buffer_to_buffer(
            &globals_staging_buffer,
            0,
            &self.resources.globals_buffer,
            0,
            std::mem::size_of::<Globals>() as u64,
        );

        // Create a thread-local-like cell for the controller to share it with closures below
        let ctx_cell = std::cell::RefCell::new(controller);

        // Delegate to GpuRenderer to calculate layout and cache glyphs
        self.gpu_renderer.try_render(
            text_layout,
            font_storage,
            // Callback: Update Texture Atlas
            &mut |updates: &[AtlasUpdate]| -> Result<(), E> {
                let mut ctx = ctx_cell.borrow_mut();
                self.resources.update_atlas(device, ctx.encoder()?, updates);
                Ok(())
            },
            // Callback: Draw standard glyphs (batched)
            &mut |instances: &[GlyphInstance<T>]| -> Result<(), E> {
                self.resources.draw_instances(
                    device,
                    &mut *ctx_cell.borrow_mut(),
                    &current_offset,
                    instances,
                )
            },
            // Callback: Draw standalone glyph (large)
            &mut |standalone: &StandaloneGlyph<T>| -> Result<(), E> {
                self.resources.draw_standalone(
                    device,
                    &mut *ctx_cell.borrow_mut(),
                    &current_offset,
                    standalone,
                )
            },
        )?;

        Ok(())
    }
}

impl WgpuResources {
    fn get_pipeline(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        // Optimistic check
        if let Some(pipeline) = self.pipelines.borrow().get(&format) {
            return pipeline.clone();
        }

        // Create new pipeline
        let instance_buffer_layout = InstanceData::vertex_buffer_layout();

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("WgpuRenderer Pipeline"),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_main"),
                buffers: std::slice::from_ref(&instance_buffer_layout),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        self.pipelines.borrow_mut().insert(format, pipeline.clone());
        pipeline
    }

    fn get_standalone_pipeline(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        if let Some(pipeline) = self.standalone_pipelines.borrow().get(&format) {
            return pipeline.clone();
        }

        let instance_buffer_layout = InstanceData::vertex_buffer_layout();

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("WgpuRenderer Standalone Pipeline"),
            layout: Some(&self.standalone_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.standalone_shader,
                entry_point: Some("vs_main"),
                buffers: std::slice::from_ref(&instance_buffer_layout),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.standalone_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        self.standalone_pipelines
            .borrow_mut()
            .insert(format, pipeline.clone());
        pipeline
    }

    /// Ensures the instance buffer has enough capacity to hold `needed_bytes`.
    ///
    /// If the buffer is too small, it creates a new one with at least double the current capacity
    /// (geometric growth) to minimize the frequency of re-allocations.
    fn ensure_instance_buffer_capacity(
        &self,
        device: &wgpu::Device,
        needed_bytes: u64,
        instance_buffer: &mut wgpu::Buffer,
    ) {
        let current_capacity = instance_buffer.size();
        if needed_bytes > current_capacity {
            let new_capacity = needed_bytes.max(current_capacity * 2);
            let new_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Instance Buffer"),
                size: new_capacity,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            *instance_buffer = new_buffer;
        }
    }

    /// Ensures that standalone resources (texture, bind group) are sufficient for the needed dimensions.
    ///
    /// # Power-of-Two Sizing
    /// To avoid recreating the texture every time the glyph size changes slightly, the texture dimensions
    /// are rounded up to the next power of two (e.g., 100x100 -> 128x128). This significantly stabilizes
    /// GPU resource churn for variable-sized large glyphs.
    fn ensure_standalone_resources(
        &self,
        device: &wgpu::Device,
        needed_width: u32,
        needed_height: u32,
    ) -> std::cell::RefMut<'_, Option<StandaloneResources>> {
        let mut resources_ref = self.standalone_resources.borrow_mut();

        let recreate = if let Some(res) = resources_ref.as_ref() {
            res.size.width < needed_width || res.size.height < needed_height
        } else {
            true
        };

        if recreate {
            let current_size = resources_ref
                .as_ref()
                .map(|r| r.size)
                .unwrap_or(wgpu::Extent3d {
                    width: 0,
                    height: 0,
                    depth_or_array_layers: 1,
                });
            let new_width = current_size.width.max(needed_width);
            let new_height = current_size.height.max(needed_height);

            let size = wgpu::Extent3d {
                width: new_width.next_power_of_two(),
                height: new_height.next_power_of_two(),
                depth_or_array_layers: 1,
            };

            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Standalone Glyph Texture"),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Standalone Bind Group"),
                layout: &self.standalone_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.globals_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                ],
            });

            *resources_ref = Some(StandaloneResources {
                texture,
                bind_group,
                size,
            });
        }

        resources_ref
    }

    /// Prepares pixel data for texture upload, handling WGPU's alignment requirements.
    ///
    /// WGPU (and underlying APIs like Vulkan/DirectX) requires that the "bytes per row" in a copy command
    /// be a multiple of **256 bytes**. If the image width doesn't match this alignment, we must
    /// copy the data into a new buffer with padding bytes added to the end of each row.
    ///
    /// - `pixel_staging`: A reusable vector to avoid allocation when padding is needed.
    fn prepare_padded_data<'a>(
        pixel_staging: &'a mut Vec<u8>,
        pixels: &'a [u8],
        width: u32,
        height: u32,
    ) -> (std::borrow::Cow<'a, [u8]>, u32) {
        let bytes_per_row = width;
        // Align to 256 bytes: (val + 255) & !255 checks the next multiple of 256.
        let padded_bytes_per_row = (bytes_per_row + 255) & !255;
        let padding = padded_bytes_per_row - bytes_per_row;

        let data = if padding == 0 {
            // No padding needed, use original data directly (zero-copy).
            std::borrow::Cow::Borrowed(pixels)
        } else {
            // Padding needed, reuse staging buffer.
            pixel_staging.clear();
            pixel_staging.reserve((padded_bytes_per_row * height) as usize);

            for row in 0..height {
                let src_start = (row * width) as usize;
                let src_end = src_start + width as usize;
                if src_end <= pixels.len() {
                    pixel_staging.extend_from_slice(&pixels[src_start..src_end]);
                    // Append zeros for alignment
                    pixel_staging.extend(std::iter::repeat_n(0, padding as usize));
                }
            }
            std::borrow::Cow::Borrowed(pixel_staging.as_slice())
        };

        (data, padded_bytes_per_row)
    }

    fn update_atlas(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        updates: &[AtlasUpdate],
    ) {
        let mut pixel_staging = self.pixel_staging.borrow_mut();

        for update in updates {
            let width = update.width as u32;
            let height = update.height as u32;

            if width == 0 || height == 0 {
                continue;
            }

            let (data, padded_bytes_per_row) =
                Self::prepare_padded_data(&mut pixel_staging, &update.pixels, width, height);

            let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Atlas Staging Buffer"),
                contents: &data,
                usage: wgpu::BufferUsages::COPY_SRC,
            });

            encoder.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &staging_buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_per_row),
                        rows_per_image: Some(height),
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: update.x as u32,
                        y: update.y as u32,
                        z: update.texture_index as u32,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    fn draw_instances<T: Into<[f32; 4]> + Copy, E>(
        &self,
        device: &wgpu::Device,
        controller: &mut impl WgpuRenderPassController<E>,
        current_offset: &std::cell::Cell<u64>,
        instances: &[GlyphInstance<T>],
    ) -> Result<(), E> {
        if instances.is_empty() {
            return Ok(());
        }

        let mut instance_buffer = self.instance_buffer.borrow_mut();

        let mut instance_data = self.instance_data_staging.borrow_mut();
        instance_data.clear();
        instance_data.extend(instances.iter().map(|inst| InstanceData {
            screen_rect: [
                inst.screen_rect.min.x,
                inst.screen_rect.min.y,
                inst.screen_rect.width(),
                inst.screen_rect.height(),
            ],
            uv_rect: [
                inst.uv_rect.min.x,
                inst.uv_rect.min.y,
                inst.uv_rect.width(),
                inst.uv_rect.height(),
            ],
            color: inst.user_data.into(),
            layer: inst.texture_index as u32,
            _padding: [0; 3],
        }));

        let instance_size = std::mem::size_of::<InstanceData>() as u64;
        let needed_bytes = current_offset.get() + instance_data.len() as u64 * instance_size;

        self.ensure_instance_buffer_capacity(device, needed_bytes, &mut instance_buffer);

        let offset = current_offset.get();
        let bytes = bytemuck::cast_slice(&instance_data);

        let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Staging Buffer"),
            contents: bytes,
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        controller.encoder()?.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &instance_buffer,
            offset,
            bytes.len() as u64,
        );

        let format = controller.format()?;
        let mut rpass = controller.create_pass()?;

        // Use cached pipeline or create new one based on format
        let pipeline = self.get_pipeline(device, format);
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, &self.globals_bind_group, &[]);
        rpass.set_vertex_buffer(
            0,
            instance_buffer.slice(offset..offset + bytes.len() as u64),
        );
        rpass.draw(0..4, 0..instance_data.len() as u32);

        current_offset.set(offset + bytes.len() as u64);
        Ok(())
    }

    fn draw_standalone<T: Into<[f32; 4]> + Copy, E>(
        &self,
        device: &wgpu::Device,
        controller: &mut impl WgpuRenderPassController<E>,
        current_offset: &std::cell::Cell<u64>,
        standalone: &StandaloneGlyph<T>,
    ) -> Result<(), E> {
        let needed_width = standalone.width as u32;
        let needed_height = standalone.height as u32;

        let resources_ref = self.ensure_standalone_resources(device, needed_width, needed_height);
        let resources = resources_ref
            .as_ref()
            .expect("Logic bug: resources_ref should be initialized.");

        // Prepare data with 256-byte alignment for copy_buffer_to_texture
        let width = standalone.width as u32;
        let height = standalone.height as u32;

        let mut pixel_staging = self.pixel_staging.borrow_mut();
        let (data, padded_bytes_per_row) =
            Self::prepare_padded_data(&mut pixel_staging, &standalone.pixels, width, height);

        let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Standalone Staging Buffer"),
            contents: &data,
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        controller.encoder()?.copy_buffer_to_texture(
            wgpu::TexelCopyBufferInfo {
                buffer: &staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::TexelCopyTextureInfo {
                texture: &resources.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        // UV calculation
        let u_max = standalone.width as f32 / resources.size.width as f32;
        let v_max = standalone.height as f32 / resources.size.height as f32;

        // Instance data for standalone
        let instance_data = InstanceData {
            screen_rect: [
                standalone.screen_rect.min.x,
                standalone.screen_rect.min.y,
                standalone.screen_rect.width(),
                standalone.screen_rect.height(),
            ],
            uv_rect: [0.0, 0.0, u_max, v_max],
            color: standalone.user_data.into(),
            layer: 0,
            _padding: [0; 3],
        };

        // Use the shared instance buffer for standalone glyphs too
        let instance_size = std::mem::size_of::<InstanceData>() as u64;
        let mut instance_buffer = self.instance_buffer.borrow_mut();
        let needed_bytes = current_offset.get() + instance_size;

        self.ensure_instance_buffer_capacity(device, needed_bytes, &mut instance_buffer);

        let offset = current_offset.get();
        let bytes = bytemuck::bytes_of(&instance_data);

        let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Standalone Instance Staging Buffer"),
            contents: bytes,
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        controller.encoder()?.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &instance_buffer,
            offset,
            bytes.len() as u64,
        );

        let format = controller.format()?;
        let mut rpass = controller.create_pass()?;

        let pipeline = self.get_standalone_pipeline(device, format);
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, &resources.bind_group, &[]);
        rpass.set_vertex_buffer(
            0,
            instance_buffer.slice(offset..offset + bytes.len() as u64),
        );
        rpass.draw(0..4, 0..1);

        current_offset.set(offset + bytes.len() as u64);
        Ok(())
    }
}
