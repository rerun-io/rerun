#import <./types.wgsl>
#import <./global_bindings.wgsl>

struct UniformBuffer {
    screen_resolution: Vec2,
    position_in_pixel: Vec2,
    extent_in_pixel: Vec2,
    _padding: Vec2,
};
@group(1) @binding(0)
var<uniform> uniforms: UniformBuffer;

@group(1) @binding(1)
var debug_texture: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) position: Vec4,
    @location(0) texcoord: Vec2,
};

@vertex
fn main_vs(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let texcoord = Vec2(f32(vertex_index / 2u), f32(vertex_index % 2u));

    // This calculation could be simplified by pre-computing things on the CPU.
    // But this is not the point here - we want to debug this and other things rapidly by editing the shader.
    let screen_portion = texcoord * (uniforms.extent_in_pixel / uniforms.screen_resolution) +
                        uniforms.position_in_pixel / uniforms.screen_resolution;
    let screen_ndc = Vec2(screen_portion.x * 2.0 - 1.0, 1.0 - screen_portion.y * 2.0);

    var out: VertexOutput;
    out.position = Vec4(screen_ndc, 0.0, 1.0);
    out.texcoord = texcoord;
    return out;
}

@fragment
fn main_fs(in: VertexOutput) -> @location(0) Vec4 {
    return Vec4(textureSample(debug_texture, nearest_sampler, in.texcoord).rgb, 1.0);
}
