#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

struct UniformBuffer {
    format: u32,
    primaries: u32,
    target_texture_size: vec2u,
};

@group(0) @binding(0)
var<uniform> uniform_buffer: UniformBuffer;

@group(0) @binding(1)
var input_texture: texture_2d<u32>;


const FORMAT_Y_UV = 0u;
const FORMAT_YUYV16 = 1u;

const PRIMARIES_BT601 = 0u;
const PRIMARIES_BT709 = 1u;


/// Returns sRGB from YUV color.
///
/// This conversion mirrors the function in `crates/store/re_types/src/datatypes/tensor_data_ext.rs`
///
/// Specifying the color standard should be exposed in the future [#3541](https://github.com/rerun-io/rerun/pull/3541)
fn srgb_from_yuv(yuv: vec3f, primaries: u32) -> vec3f {
    // rescale YUV values
    let y = (yuv[0] - 16.0) / 219.0;
    let u = (yuv[1] - 128.0) / 224.0;
    let v = (yuv[2] - 128.0) / 224.0;

    var rgb: vec3f;

    switch (primaries) {
        // BT.601 (aka. SDTV, aka. Rec.601). wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.601_conversion
        // Also note according to https://en.wikipedia.org/wiki/SRGB#sYCC_extended-gamut_transformation
        // > Although the RGB color primaries are based on BT.709,
        // > the equations for transformation from sRGB to sYCC and vice versa are based on BT.601.
        case PRIMARIES_BT601: {
            rgb.r = y + 1.402 * v;
            rgb.g = y - 0.344 * u - 0.714 * v;
            rgb.b = y + 1.772 * u;
        }

        // BT.709 (aka. HDTV, aka. Rec.709). wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.709_conversion
        case PRIMARIES_BT709: {
            rgb.r = y + 1.575 * v;
            rgb.g = y - 0.187 * u - 0.468 * v;
            rgb.b = y + 1.856 * u;
        }

        default: {
            rgb = ERROR_RGBA.rgb;
        }
    }

    return clamp(rgb, vec3f(0.0), vec3f(1.0));
}

/// Extracts YUV data from a chroma subsampling encoded texture at specific coordinates.
///
/// See also `enum ChromaSubsamplingPixelFormat` in `chroma_subsampling_converter.rs for a specification of
/// the expected data layout.
fn decode_chroma_subsampling_format_to_yuv(format: u32, texture: texture_2d<u32>, coords: vec2f) -> vec3f {
    let texture_dim = vec2f(textureDimensions(texture).xy);
    var yuv: vec3f;

    switch (format)  {
        case FORMAT_Y_UV: {
            let uv_offset = u32(floor(texture_dim.y / 1.5));
            let uv_row = u32(coords.y / 2);
            var uv_col = u32(coords.x / 2) * 2u;

            yuv[0] = f32(textureLoad(texture, vec2u(coords), 0).r);
            yuv[1] = f32(textureLoad(texture, vec2u(u32(uv_col), uv_offset + uv_row), 0).r);
            yuv[2] = f32(textureLoad(texture, vec2u((u32(uv_col) + 1u), uv_offset + uv_row), 0).r);
        }

        case FORMAT_YUYV16: {
            // texture is 2 * width * height
            // every 4 bytes is 2 pixels
            let uv_row = u32(coords.y);
            // multiply by 2 because the width is multiplied by 2
            let y_col = u32(coords.x) * 2u;
            yuv[0] = f32(textureLoad(texture, vec2u(y_col, uv_row), 0).r);

            // at odd pixels we're in the second half of the yuyu block, offset back by 2
            let uv_col = y_col - u32(coords.x % 2) * 2u;
            yuv[1] = f32(textureLoad(texture, vec2u(uv_col + 1u, uv_row), 0).r);
            yuv[2] = f32(textureLoad(texture, vec2u(uv_col + 3u, uv_row), 0).r);
        }

        default: {
            yuv = vec3f(0.0, 0.0, 0.0); // ERROR_RGBA doesn't apply here.
        }
    }

    return yuv;
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    let coords = vec2f(uniform_buffer.target_texture_size) * in.texcoord;

    let yuv = decode_chroma_subsampling_format_to_yuv(uniform_buffer.format, input_texture, coords);
    let rgb = srgb_from_yuv(yuv, uniform_buffer.primaries);

    return vec4f(rgb, 1.0);
}
