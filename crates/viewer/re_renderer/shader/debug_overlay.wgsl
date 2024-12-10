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
    screen_resolution: vec2f,
    position_in_pixel: vec2f,
    extent_in_pixel: vec2f,
    mode: u32,
    _padding: u32,
};
@group(1) @binding(0)
var<uniform> uniforms: UniformBuffer;

@group(1) @binding(1)
var debug_texture_float: texture_2d<f32>;
@group(1) @binding(2)
var debug_texture_uint: texture_2d<u32>;

// Mode options, see `DebugOverlayMode` in `debug_overlay.rs`
const ShowFloatTexture: u32 = 0u;
const ShowUintTexture: u32 = 1u;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) texcoord: vec2f,
};

@vertex
fn main_vs(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let texcoord = vec2f(f32(vertex_index / 2u), f32(vertex_index % 2u));

    // This calculation could be simplified by pre-computing things on the CPU.
    // But this is not the point here - we want to debug this and other things rapidly by editing the shader.
    let screen_fraction = texcoord * (uniforms.extent_in_pixel / uniforms.screen_resolution) +
                        uniforms.position_in_pixel / uniforms.screen_resolution;
    let screen_ndc = vec2f(screen_fraction.x * 2.0 - 1.0, 1.0 - screen_fraction.y * 2.0);

    var out: VertexOutput;
    out.position = vec4f(screen_ndc, 0.0, 1.0);
    out.texcoord = texcoord;
    return out;
}

@fragment
fn main_fs(in: VertexOutput) -> @location(0) vec4f {
    if uniforms.mode == ShowFloatTexture {
        return vec4f(textureSample(debug_texture_float, nearest_sampler_clamped, in.texcoord).rgb, 1.0);
    } else if uniforms.mode == ShowUintTexture {
        let coords = vec2i(in.texcoord * vec2f(textureDimensions(debug_texture_uint).xy));
        let raw_values = textureLoad(debug_texture_uint, coords, 0);

        let num_color_levels = 20u;
        let mapped_values = vec4f(raw_values % num_color_levels) / f32(num_color_levels - 1u);

        return vec4f(mapped_values.rgb, 1.0);
    } else {
        return vec4f(1.0, 0.0, 1.0, 1.0);
    }
}
