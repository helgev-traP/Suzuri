use std::num::NonZeroUsize;

use image::{ImageBuffer, Rgba};
use wgfont::{
    font_storage::FontStorage,
    renderer::{gpu_renderer::GpuCacheConfig, wgpu_renderer::WgpuRenderer},
};

mod example_common;
use example_common::{WIDTH, build_text_data, load_fonts, make_layout_config};

#[allow(clippy::unwrap_used)]
fn main() {
    pollster::block_on(run());
}

#[allow(clippy::unwrap_used)]
async fn run() {
    // 1. Setup wgpu
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        flags: wgpu::InstanceFlags::default(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .expect("Failed to find an appropriate adapter");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            ..Default::default()
        })
        .await
        .expect("Failed to create device");

    let device = std::sync::Arc::new(device);
    let queue = std::sync::Arc::new(queue);

    // 2. Setup WgpuRenderer
    #[allow(clippy::unwrap_used)]
    let configs = vec![
        GpuCacheConfig {
            tile_size: NonZeroUsize::new(32).unwrap(),
            tiles_per_axis: NonZeroUsize::new(16).unwrap(),
            texture_size: NonZeroUsize::new(512).unwrap(),
        },
        GpuCacheConfig {
            tile_size: NonZeroUsize::new(64).unwrap(),
            tiles_per_axis: NonZeroUsize::new(8).unwrap(),
            texture_size: NonZeroUsize::new(512).unwrap(),
        },
    ];

    let texture_format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = WgpuRenderer::new(&device, &configs, &[texture_format]);

    // 3. Setup Text Layout
    let config = make_layout_config(Some(WIDTH), None);

    let mut font_storage = FontStorage::new();
    let (heading_font, body_font, mono_font) = load_fonts(&mut font_storage);
    let data = build_text_data(heading_font, body_font, mono_font);

    let layout_timer = std::time::Instant::now();
    let layout = data.layout(&config, &mut font_storage);
    let layout_elapsed = layout_timer.elapsed();

    println!(
        "Layout: {:.2}x{:.2} lines={} (elapsed: {:.2?})",
        layout.total_width,
        layout.total_height,
        layout.lines.len(),
        layout_elapsed
    );

    let width = WIDTH as u32;
    let height = layout.total_height.ceil() as u32;

    // 4. Create Target Texture
    let target_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Target Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: texture_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let target_view = target_texture.create_view(&wgpu::TextureViewDescriptor::default());

    // 5. Render
    let u32_size = std::mem::size_of::<u32>() as u32;
    let output_buffer_size = (u32_size * width * height) as wgpu::BufferAddress;
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: output_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut measurements = Vec::new();

    for i in 0..2 {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let _rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        let start = std::time::Instant::now();
        renderer.render(
            &layout,
            &mut font_storage,
            &device,
            &mut encoder,
            &target_view,
            [width as f32, height as f32],
        );
        measurements.push(start.elapsed());

        if i == 1 {
            // 6. Copy to Buffer (last pass only)
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    aspect: wgpu::TextureAspect::All,
                    texture: &target_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &output_buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(u32_size * width),
                        rows_per_image: Some(height),
                    },
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }

        queue.submit(Some(encoder.finish()));
    }

    println!(
        "Render (1st): {}x{} (elapsed: {:.2?})",
        width, height, measurements[0]
    );
    println!(
        "Render (2nd): {}x{} (elapsed: {:.2?})",
        width, height, measurements[1]
    );

    // 7. Read Buffer and Save
    let buffer_slice = output_buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |v| sender.send(v).unwrap());

    instance.poll_all(true);
    receiver.recv().unwrap().unwrap();

    let data = buffer_slice.get_mapped_range();
    let buffer = data.to_vec();
    drop(data);
    output_buffer.unmap();

    std::fs::create_dir_all("debug").expect("failed to create debug directory");

    let img_buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, buffer).expect("failed to create image buffer");

    let output_path = "debug/wgpu_renderer_text.png";
    img_buffer.save(output_path).expect("failed to save image");

    println!("Saved: {}", output_path);
}
