#import <./types.wgsl>
#import <./global_bindings.wgsl>

struct UniformBuffer {
    screen_resolution: Vec2,
    position_in_pixel: Vec2,
    extent_in_pixel: Vec2,
    mode: u32,
    _padding: u32,
};
@group(1) @binding(0)
var<uniform> uniforms: UniformBuffer;

@group(1) @binding(1)
var debug_texture_float: texture_2d<f32>;
@group(1) @binding(2)
var debug_texture_uint: texture_2d<u32>;

// Mode options, see `DebugOverlayMode`
const ShowFloatTexture: u32 = 0u;
const ShowUintTexture: u32 = 1u;

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
    if uniforms.mode == ShowFloatTexture {
        return Vec4(textureSample(debug_texture_float, nearest_sampler, in.texcoord).rgb, 1.0);
    } else if uniforms.mode == ShowUintTexture {
        let num_color_levels = 20u;
        let coords = IVec2(in.texcoord * Vec2(textureDimensions(debug_texture_uint).xy));
        return Vec4(Vec3(textureLoad(debug_texture_uint, coords, 0).rgb % num_color_levels) / f32(num_color_levels - 1u), 1.0);
    } else {
        return Vec4(1.0, 0.0, 1.0, 1.0);
    }
}
