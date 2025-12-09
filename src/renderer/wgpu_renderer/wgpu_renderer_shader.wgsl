struct Globals {
    screen_size: vec2<f32>,
};
@group(0) @binding(0) var<uniform> globals: Globals;
@group(0) @binding(1) var font_sampler: sampler;
@group(0) @binding(2) var font_texture: texture_2d_array<f32>;

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
}

struct InstanceInput {
    @location(0) screen_rect: vec4<f32>,
    @location(1) uv_rect: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) layer: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) layer: u32,
}

@vertex
fn vs_main(model: VertexInput, instance: InstanceInput) -> VertexOutput {
    let idx = model.vertex_index;
    // 0: (0, 0), 1: (0, 1), 2: (1, 0), 3: (1, 1)
    let x = f32(idx & 1u);
    let y = f32(idx >> 1u);

    let screen_pos = instance.screen_rect.xy + vec2<f32>(x, y) * instance.screen_rect.zw;
    let uv_pos = instance.uv_rect.xy + vec2<f32>(x, y) * instance.uv_rect.zw;

    // Convert to clip space (-1 to 1)
    // screen_pos is in pixels (0 to width, 0 to height)
    // x: 0..w -> -1..1 => x / w * 2 - 1
    // y: 0..h -> 1..-1 => -(y / h * 2 - 1) = 1 - y / h * 2
    
    let clip_x = (screen_pos.x / globals.screen_size.x) * 2.0 - 1.0;
    let clip_y = 1.0 - (screen_pos.y / globals.screen_size.y) * 2.0;

    var out: VertexOutput;
    out.clip_position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    out.tex_coords = uv_pos;
    out.color = instance.color;
    out.layer = instance.layer;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = textureSample(font_texture, font_sampler, in.tex_coords, i32(in.layer)).r;
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
