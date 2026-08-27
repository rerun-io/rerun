#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>
#import <yuv_srgb_conversion.wgsl>

struct UniformBuffer {
    yuv_matrix_coefficients: u32,
    yuv_range: u32,
    target_texture_size: vec2u,
};

@group(0) @binding(0)
var<uniform> uniform_buffer: UniformBuffer;

/// The luma plane of an NV12 texture (or an equivalent standalone texture).
@group(0) @binding(1)
var y_texture: texture_2d<f32>;

/// The interleaved chroma plane at half resolution.
@group(0) @binding(2)
var uv_texture: texture_2d<f32>;

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    let coords = vec2u(vec2f(uniform_buffer.target_texture_size) * in.texcoord);

    // The planes can be padded slightly larger than the target (even-size padding of
    // odd videos), which just means we never sample their last row/column.
    // `srgb_from_yuv` expects 0-255 values, unorm loads give 0-1.
    let y = textureLoad(y_texture, coords, 0).r * 255.0;
    let uv = textureLoad(uv_texture, coords / vec2u(2u), 0).rg * 255.0;

    let rgb = srgb_from_yuv(vec3f(y, uv), uniform_buffer.yuv_matrix_coefficients, uniform_buffer.yuv_range);

    return vec4f(rgb, 1.0);
}
