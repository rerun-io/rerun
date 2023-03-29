// Debug overlay shader
//
// Works together with `debug_overlay.rs` to display a texture on top of the screen.
// It is meant to be used as last part of the compositor phase in order to present the debug output unfiltered.
// It's sole purpose is for developing new rendering features and it should not be used in production!
//
// The fragment shader is a blueprint for handling different texture outputs.
// *Do* edit it on the fly for debugging purposes!

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
const SHOW_FLOAT_TEXTURE: u32 = 0u;
const SHOW_UINT_TEXTURE: u32 = 1u;

struct VertexOutput {
    @builtin(position) position: Vec4,
    @location(0) texcoord: Vec2,
};

@vertex
fn main_vs(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let texcoord = Vec2(f32(vertex_index / 2u), f32(vertex_index % 2u));

    // This calculation could be simplified by pre-computing things on the CPU.
    // But this is not the point here - we want to debug this and other things rapidly by editing the shader.
    let screen_fraction = texcoord * (uniforms.extent_in_pixel / uniforms.screen_resolution) +
                        uniforms.position_in_pixel / uniforms.screen_resolution;
    let screen_ndc = Vec2(screen_fraction.x * 2.0 - 1.0, 1.0 - screen_fraction.y * 2.0);

    var out: VertexOutput;
    out.position = Vec4(screen_ndc, 0.0, 1.0);
    out.texcoord = texcoord;
    return out;
}

@fragment
fn main_fs(in: VertexOutput) -> @location(0) Vec4 {
    if uniforms.mode == SHOW_FLOAT_TEXTURE {
        return Vec4(textureSample(debug_texture_float, nearest_sampler, in.texcoord).rgb, 1.0);
    } else if uniforms.mode == SHOW_UINT_TEXTURE {
        let coords = IVec2(in.texcoord * Vec2(textureDimensions(debug_texture_uint).xy));
        let raw_values = textureLoad(debug_texture_uint, coords, 0);

        let num_color_levels = 20u;
        let mapped_values = (raw_values % num_color_levels) / f32(num_color_levels - 1u);

        return Vec4(mapped_values.rgb, 1.0);
    } else {
        return Vec4(1.0, 0.0, 1.0, 1.0);
    }
}
