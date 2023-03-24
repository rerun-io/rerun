#import <./types.wgsl>
#import <./global_bindings.wgsl>

struct UniformBuffer {
    position_in_pixel: UVec2,
    extent_in_pixel: UVec2,
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

    var out: VertexOutput;
    out.position = Vec4(texcoord.x * 2.0 - 1.0, 2.0 - texcoord.y * 2.0, 0.0, 1.0);
    out.texcoord = texcoord;
    return out;
}

@fragment
fn main_fs(in: VertexOutput) -> @location(0) Vec4 {
    return textureSample(debug_texture, nearest_sampler, in.texcoord);
}
