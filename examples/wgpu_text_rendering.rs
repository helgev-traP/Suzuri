use std::collections::HashSet;
use std::num::NonZeroUsize;

use euclid::num::Ceil;
use fxhash::FxBuildHasher;
use image::{ImageBuffer, Rgba};
use wgfont::{
    font_storage::FontStorage,
    fontdb::{self, Family, Query},
    renderer::{
        gpu_renderer::GlyphAtlasConfig,
        wgpu_renderer::{ToInstance, WgpuRenderer},
    },
    text::{HorizontalAlign, TextData, TextElement, TextLayoutConfig, VerticalAlign, WrapStyle},
};

fn make_config(max_width: Option<f32>, max_height: Option<f32>) -> TextLayoutConfig {
    let mut word_separators: HashSet<char, FxBuildHasher> =
        HashSet::with_hasher(FxBuildHasher::default());
    word_separators.insert(' ');
    word_separators.insert('\t');
    word_separators.insert(',');
    word_separators.insert('.');

    let mut linebreak_char: HashSet<char, FxBuildHasher> =
        HashSet::with_hasher(FxBuildHasher::default());
    linebreak_char.insert('\n');

    TextLayoutConfig {
        max_width,
        max_height,
        horizontal_align: HorizontalAlign::Left,
        vertical_align: VerticalAlign::Top,
        line_height_scale: 1.0,
        wrap_style: WrapStyle::WordWrap,
        wrap_hard_break: true,
        word_separators,
        linebreak_char,
    }
}

fn pick_system_font(font_storage: &mut FontStorage) -> fontdb::ID {
    font_storage.load_system_fonts();
    assert!(
        !font_storage.is_empty(),
        "system fonts are required for the text layout test"
    );

    const FAMILIES: &[Family<'_>] = &[Family::SansSerif];
    let query = Query {
        families: FAMILIES,
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };

    if let Some((font_id, _)) = font_storage.query(&query) {
        return font_id;
    }

    font_storage
        .faces()
        .next()
        .map(|face| face.id)
        .expect("no usable fonts registered in FontStorage")
}

// Define a color type for the example
#[derive(Clone, Copy, Debug)]
struct TextColor {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

impl ToInstance for TextColor {
    fn to_color(&self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

#[allow(clippy::unwrap_used)]
fn main() {
    pollster::block_on(run());
}

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
    let configs = vec![
        GlyphAtlasConfig {
            tile_size: NonZeroUsize::new(32).unwrap(),
            tiles_per_axis: NonZeroUsize::new(16).unwrap(),
            texture_size: NonZeroUsize::new(512).unwrap(),
        },
        GlyphAtlasConfig {
            tile_size: NonZeroUsize::new(64).unwrap(),
            tiles_per_axis: NonZeroUsize::new(8).unwrap(),
            texture_size: NonZeroUsize::new(512).unwrap(),
        },
    ];

    let texture_format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = WgpuRenderer::new(device.clone(), configs, texture_format);

    // 3. Setup Text Layout
    let config = {
        let max_width = Some(3200.0);
        let max_height = None;
        make_config(max_width, max_height)
    };

    let mut font_storage = FontStorage::new();
    let font_id = pick_system_font(&mut font_storage);

    let mut data = TextData::new();
    // 1. Title
    data.append(TextElement {
        font_id,
        font_size: 192.0,
        content: "Typography Showcase\n".into(),
        user_data: TextColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }, // White
    });
    data.append(TextElement {
        font_id,
        font_size: 96.0,
        content: "\n".into(),
        user_data: TextColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    });

    // 2. Introduction (Mixed colors per word)
    let intro_text = "This example demonstrates the capabilities of the WgpuRenderer. It supports efficient batching, dynamic atlas management, and standalone rendering for large glyphs.\n\n";
    let words: Vec<&str> = intro_text.split(' ').collect();

    for (i, word) in words.iter().enumerate() {
        let r = 0.5 + (i as f32 * 0.1).sin().abs() * 0.5;
        let g = 0.5 + (i as f32 * 0.2).cos().abs() * 0.5;
        let b = 0.5 + (i as f32 * 0.3).sin().abs() * 0.5;

        // Add space after word unless it's the last one or contains newline
        let content = if word.contains('\n') || i == words.len() - 1 {
            word.to_string()
        } else {
            format!("{} ", word)
        };

        data.append(TextElement {
            font_id,
            font_size: 72.0,
            content,
            user_data: TextColor { r, g, b, a: 1.0 },
        });
    }

    // 3. Features List (Mixed styles)
    data.append(TextElement {
        font_id,
        font_size: 96.0,
        content: "Key Features:\n".into(),
        user_data: TextColor {
            r: 1.0,
            g: 0.8,
            b: 0.4,
            a: 1.0,
        }, // Gold
    });

    // Feature 1: Colors
    data.append(TextElement {
        font_id,
        font_size: 80.0,
        content: "• Rich Colors: ".into(),
        user_data: TextColor {
            r: 0.9,
            g: 0.9,
            b: 0.9,
            a: 1.0,
        },
    });
    data.append(TextElement {
        font_id,
        font_size: 80.0,
        content: "Red, ".into(),
        user_data: TextColor {
            r: 1.0,
            g: 0.4,
            b: 0.4,
            a: 1.0,
        },
    });
    data.append(TextElement {
        font_id,
        font_size: 80.0,
        content: "Green, ".into(),
        user_data: TextColor {
            r: 0.4,
            g: 1.0,
            b: 0.4,
            a: 1.0,
        },
    });
    data.append(TextElement {
        font_id,
        font_size: 80.0,
        content: "and Blue.\n".into(),
        user_data: TextColor {
            r: 0.4,
            g: 0.4,
            b: 1.0,
            a: 1.0,
        },
    });

    // Feature 2: Sizes
    data.append(TextElement {
        font_id,
        font_size: 80.0,
        content: "• Varied Sizes: ".into(),
        user_data: TextColor {
            r: 0.9,
            g: 0.9,
            b: 0.9,
            a: 1.0,
        },
    });
    data.append(TextElement {
        font_id,
        font_size: 40.0,
        content: "Tiny ".into(),
        user_data: TextColor {
            r: 0.7,
            g: 0.7,
            b: 0.7,
            a: 1.0,
        },
    });
    data.append(TextElement {
        font_id,
        font_size: 80.0,
        content: "Normal ".into(),
        user_data: TextColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    });
    data.append(TextElement {
        font_id,
        font_size: 120.0,
        content: "Large\n".into(),
        user_data: TextColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    });

    // 4. Lorem Ipsum (Long text for wrapping)
    data.append(TextElement {
        font_id,
        font_size: 64.0,
        content: "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.".into(),
        user_data: TextColor { r: 0.6, g: 0.6, b: 0.7, a: 1.0 }, // Muted Blue-Gray
    });

    let layout = data.layout(&config, &mut font_storage);

    let width = 3200;
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

    renderer.render(
        &layout,
        &mut font_storage,
        &target_view,
        &mut encoder,
        [width as f32, height as f32],
    );

    // 6. Copy to Buffer
    let u32_size = std::mem::size_of::<u32>() as u32;
    let output_buffer_size = (u32_size * width * height) as wgpu::BufferAddress;
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: output_buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

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

    queue.submit(Some(encoder.finish()));

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

    img_buffer
        .save("debug/wgpu_renderer_text.png")
        .expect("failed to save image");

    println!("Saved rendered image to debug/wgpu_renderer_text.png");
}
