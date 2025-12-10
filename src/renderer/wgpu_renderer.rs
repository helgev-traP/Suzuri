use super::gpu_renderer::{
    AtlasUpdate, GlyphInstance, GpuCacheConfig, GpuRenderer, StandaloneGlyph,
};
use crate::font_storage::FontStorage;
use crate::text::TextLayout;
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct InstanceData {
    pub screen_rect: [f32; 4], // x, y, w, h
    pub uv_rect: [f32; 4],     // u, v, w, h
    pub color: [f32; 4],
    pub layer: u32,
    pub _padding: [u32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Globals {
    screen_size: [f32; 2],
    _padding: [f32; 2],
}

/// A text renderer using `wgpu` for hardware-accelerated rendering.
///
/// This renderer efficiently draws text using a texture atlas and GPU instancing.
/// It supports caching glyphs on the GPU and batching draw calls.
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
/// ```rust
/// let renderer = WgpuRenderer::new(
///     &device,
///     &cache_configs,
///     &[wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm] // Pre-compile these
/// );
/// ```
///
/// # Usage
/// 1. Initialize with `WgpuRenderer::new`.
/// 2. Prepare text layout using `FontSystem`.
/// 3. Call `render` inside your generic render pass.
///
/// ```no_run
/// renderer.render(
///     &device,
///     &layout,
///     &mut font_storage,
///     &texture_view,
///     &mut encoder,
///     [screen_width, screen_height],
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

struct WgpuResources {
    pipelines: std::cell::RefCell<HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>>,
    standalone_pipelines: std::cell::RefCell<HashMap<wgpu::TextureFormat, wgpu::RenderPipeline>>,

    pipeline_layout: wgpu::PipelineLayout,
    standalone_pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    standalone_shader: wgpu::ShaderModule,

    atlas_texture: wgpu::Texture,
    sampler: wgpu::Sampler,
    instance_buffer: std::cell::RefCell<wgpu::Buffer>,
    _bind_group_layout: wgpu::BindGroupLayout,
    standalone_bind_group_layout: wgpu::BindGroupLayout,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    standalone_resources: std::cell::RefCell<Option<StandaloneResources>>,
}

struct StandaloneResources {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: wgpu::Extent3d,
}

const SHADER: &str = include_str!("wgpu_renderer/wgpu_renderer_shader.wgsl");

const STANDALONE_SHADER: &str = include_str!("wgpu_renderer/wgpu_renderer_standalone.wgsl");

impl WgpuRenderer {
    pub fn new(
        device: &wgpu::Device,
        configs: &[GpuCacheConfig],
        formats: &[wgpu::TextureFormat],
    ) -> Self {
        let gpu_renderer = GpuRenderer::new(configs);

        // Calculate max dimensions and layers
        let max_width = configs
            .iter()
            .map(|c| c.texture_size.get())
            .max()
            .unwrap_or(512) as u32;
        let max_height = configs
            .iter()
            .map(|c| c.texture_size.get())
            .max()
            .unwrap_or(512) as u32;
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

        let instance_capacity = 1024;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer"),
            size: (instance_capacity * std::mem::size_of::<InstanceData>()) as u64,
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

    pub fn clear_cache(&mut self) {
        self.gpu_renderer.clear_cache();
    }

    pub fn render<T: Into<[f32; 4]> + Copy>(
        &mut self,
        layout: &TextLayout<T>,
        font_storage: &mut FontStorage,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        screen_size: [f32; 2],
    ) {
        // Reset offset at the beginning of the frame
        let current_offset = std::cell::Cell::new(0);

        // Update globals
        let globals = Globals {
            screen_size,
            _padding: [0.0; 2],
        };
        let globals_staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Globals Staging Buffer"),
            contents: bytemuck::bytes_of(&globals),
            usage: wgpu::BufferUsages::COPY_SRC,
        });
        encoder.copy_buffer_to_buffer(
            &globals_staging_buffer,
            0,
            &self.resources.globals_buffer,
            0,
            std::mem::size_of::<Globals>() as u64,
        );

        let encoder_cell = std::cell::RefCell::new(encoder);

        self.gpu_renderer.render(
            layout,
            font_storage,
            &mut |updates: &[AtlasUpdate]| {
                self.resources
                    .update_atlas(device, &mut encoder_cell.borrow_mut(), updates);
            },
            &mut |instances: &[GlyphInstance<T>]| {
                self.resources.draw_instances(
                    device,
                    &mut encoder_cell.borrow_mut(),
                    view,
                    &current_offset,
                    instances,
                );
            },
            &mut |standalone: &StandaloneGlyph<T>| {
                self.resources.draw_standalone(
                    device,
                    &mut encoder_cell.borrow_mut(),
                    view,
                    &current_offset,
                    standalone,
                );
            },
        );
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
        let instance_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
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
            ],
        };

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

        let instance_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
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
            ],
        };

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

    fn update_atlas(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        updates: &[AtlasUpdate],
    ) {
        for update in updates {
            let width = update.width as u32;
            let height = update.height as u32;

            if width == 0 || height == 0 {
                continue;
            }

            let bytes_per_row = width;
            let padded_bytes_per_row = (bytes_per_row + 255) & !255;
            let padding = padded_bytes_per_row - bytes_per_row;

            let data = if padding == 0 {
                std::borrow::Cow::Borrowed(&update.pixels)
            } else {
                let mut padded = Vec::with_capacity((padded_bytes_per_row * height) as usize);
                for row in 0..height {
                    let src_start = (row * width) as usize;
                    let src_end = src_start + width as usize;
                    if src_end <= update.pixels.len() {
                        padded.extend_from_slice(&update.pixels[src_start..src_end]);
                        padded.extend(std::iter::repeat_n(0, padding as usize));
                    }
                }
                std::borrow::Cow::Owned(padded)
            };

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

    fn draw_instances<T: Into<[f32; 4]> + Copy>(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        current_offset: &std::cell::Cell<u64>,
        instances: &[GlyphInstance<T>],
    ) {
        if instances.is_empty() {
            return;
        }

        let mut instance_buffer = self.instance_buffer.borrow_mut();

        let instance_data: Vec<InstanceData> = instances
            .iter()
            .map(|inst| InstanceData {
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
            })
            .collect();

        let instance_size = std::mem::size_of::<InstanceData>() as u64;
        let current_capacity = instance_buffer.size();
        let needed_bytes = current_offset.get() + instance_data.len() as u64 * instance_size;

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

        let offset = current_offset.get();
        let bytes = bytemuck::cast_slice(&instance_data);

        let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Staging Buffer"),
            contents: bytes,
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        encoder.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &instance_buffer,
            offset,
            bytes.len() as u64,
        );

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Text Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Use cached pipeline or create new one based on format
        let pipeline = self.get_pipeline(device, view.texture().format());
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, &self.globals_bind_group, &[]);
        rpass.set_vertex_buffer(
            0,
            instance_buffer.slice(offset..offset + bytes.len() as u64),
        );
        rpass.draw(0..4, 0..instance_data.len() as u32);

        current_offset.set(offset + bytes.len() as u64);
    }

    fn draw_standalone<T: Into<[f32; 4]> + Copy>(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        current_offset: &std::cell::Cell<u64>,
        standalone: &StandaloneGlyph<T>,
    ) {
        let mut resources_ref = self.standalone_resources.borrow_mut();
        let mut instance_buffer = self.instance_buffer.borrow_mut();

        let needed_width = standalone.width as u32;
        let needed_height = standalone.height as u32;

        let mut recreate = false;
        if let Some(res) = resources_ref.as_ref() {
            if res.size.width < needed_width || res.size.height < needed_height {
                recreate = true;
            }
        } else {
            recreate = true;
        }

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
                width: new_width,
                height: new_height,
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

        let resources = resources_ref.as_ref().expect(
            "Logic bug: resources_ref should be initialized. If it was previously None, the 'recreate' flag ensures it is initialized above.",
        );

        // Prepare data with 256-byte alignment for copy_buffer_to_texture
        let width = standalone.width as u32;
        let height = standalone.height as u32;
        let bytes_per_row = width;
        let padded_bytes_per_row = (bytes_per_row + 255) & !255;
        let padding = padded_bytes_per_row - bytes_per_row;

        let data = if padding == 0 {
            std::borrow::Cow::Borrowed(&standalone.pixels)
        } else {
            let mut padded = Vec::with_capacity((padded_bytes_per_row * height) as usize);
            for row in 0..height {
                let src_start = (row * width) as usize;
                let src_end = src_start + width as usize;
                padded.extend_from_slice(&standalone.pixels[src_start..src_end]);
                padded.extend(std::iter::repeat_n(0, padding as usize));
            }
            std::borrow::Cow::Owned(padded)
        };

        let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Standalone Staging Buffer"),
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
        let current_capacity = instance_buffer.size();
        let needed_bytes = current_offset.get() + instance_size;

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

        let offset = current_offset.get();
        let bytes = bytemuck::bytes_of(&instance_data);

        let staging_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Standalone Instance Staging Buffer"),
            contents: bytes,
            usage: wgpu::BufferUsages::COPY_SRC,
        });

        encoder.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            &instance_buffer,
            offset,
            bytes.len() as u64,
        );

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Standalone Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        let pipeline = self.get_standalone_pipeline(device, view.texture().format());
        rpass.set_pipeline(&pipeline);
        rpass.set_bind_group(0, &resources.bind_group, &[]);
        rpass.set_vertex_buffer(
            0,
            instance_buffer.slice(offset..offset + bytes.len() as u64),
        );
        rpass.draw(0..4, 0..1);

        current_offset.set(offset + bytes.len() as u64);
    }
}
