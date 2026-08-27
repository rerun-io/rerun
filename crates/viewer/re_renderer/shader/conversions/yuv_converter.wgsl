#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>
#import <yuv_srgb_conversion.wgsl>

struct UniformBuffer {
    yuv_layout: u32,
    yuv_matrix_coefficients: u32,
    target_texture_size: vec2u,
    yuv_range: u32,

    _padding: vec3f, // Satisfy `DownlevelFlags::BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED`
};

@group(0) @binding(0)
var<uniform> uniform_buffer: UniformBuffer;

@group(0) @binding(1)
var input_texture: texture_2d<u32>;

// see `enum YuvPixelLayout`.
const YUV_LAYOUT_Y_U_V444 = 0u;
const YUV_LAYOUT_Y_U_V422 = 1u;
const YUV_LAYOUT_Y_U_V420 = 2u;
const YUV_LAYOUT_Y_UV420 = 100u;
const YUV_LAYOUT_YUYV422 = 200u;
const YUV_LAYOUT_Y400 = 300u;

/// Extracts YUV data from a chroma subsampling encoded texture at specific coordinates.
///
/// See also `enum YuvPixelLayout` in `yuv_converter.rs for a specification of
/// the expected data layout.
fn sample_yuv(yuv_layout: u32, texture: texture_2d<u32>, coords: vec2u, target_texture_size: vec2u) -> vec3f {
    let texture_dim = vec2f(textureDimensions(texture).xy);
    var yuv: vec3f;

    switch (yuv_layout)  {
        case YUV_LAYOUT_Y_U_V444: {
            // Just 3 planes under each other.
            yuv[0] = f32(textureLoad(texture, coords, 0).r);
            yuv[1] = f32(textureLoad(texture, vec2u(coords.x, coords.y + target_texture_size.y), 0).r);
            yuv[2] = f32(textureLoad(texture, vec2u(coords.x, coords.y + target_texture_size.y * 2u), 0).r);
        }

        case YUV_LAYOUT_Y_U_V422: {
            // A large Y plane, followed by a UV plane with half the horizontal resolution,
            // every row contains two u/v rows.
            yuv[0] = f32(textureLoad(texture, coords, 0).r);
            // UV coordinate on its own plane:
            let uv_coord = vec2u(coords.x / 2u, coords.y);
            // UV coordinate on the data texture, ignoring offset from previous planes.
            // Each texture row contains two UV rows
            let uv_col = uv_coord.x + (uv_coord.y % 2) * target_texture_size.x / 2u;
            let uv_row = uv_coord.y / 2u;

            yuv[1] = f32(textureLoad(texture, vec2u(uv_col, uv_row + target_texture_size.y), 0).r);
            yuv[2] = f32(textureLoad(texture, vec2u(uv_col, uv_row + target_texture_size.y + target_texture_size.y / 2u), 0).r);
        }

        case YUV_LAYOUT_Y_U_V420: {
            // A large Y plane, followed by a UV plane with half the horizontal & vertical resolution,
            // every row contains two u/v rows and there's only half as many.
            yuv[0] = f32(textureLoad(texture, coords, 0).r);
            // UV coordinate on its own plane:
            let uv_coord = vec2u(coords.x / 2u, coords.y / 2u);
            // UV coordinate on the data texture, ignoring offset from previous planes.
            // Each texture row contains two UV rows
            let uv_col = uv_coord.x + (uv_coord.y % 2) * (target_texture_size.x / 2u);
            let uv_row = uv_coord.y / 2u;

            yuv[1] = f32(textureLoad(texture, vec2u(uv_col, uv_row + target_texture_size.y), 0).r);
            yuv[2] = f32(textureLoad(texture, vec2u(uv_col, uv_row + target_texture_size.y + target_texture_size.y / 4u), 0).r);
        }

        case YUV_LAYOUT_Y400 {
            yuv[0] = f32(textureLoad(texture, coords, 0).r);
            yuv[1] = 128.0;
            yuv[2] = 128.0;
        }

        case YUV_LAYOUT_Y_UV420: {
            let uv_offset = u32(floor(texture_dim.y / 1.5));
            let uv_row = (coords.y / 2u);
            var uv_col = (coords.x / 2u) * 2u;

            yuv[0] = f32(textureLoad(texture, coords, 0).r);
            yuv[1] = f32(textureLoad(texture, vec2u(uv_col, uv_offset + uv_row), 0).r);
            yuv[2] = f32(textureLoad(texture, vec2u((uv_col + 1u), uv_offset + uv_row), 0).r);
        }

        case YUV_LAYOUT_YUYV422: {
            // texture is 2 * width * height
            // every 4 bytes is 2 pixels
            let uv_row = coords.y;
            // multiply by 2 because the width is multiplied by 2
            let y_col = coords.x * 2u;
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
    let coords = vec2u(vec2f(uniform_buffer.target_texture_size) * in.texcoord);

    let yuv = sample_yuv(uniform_buffer.yuv_layout, input_texture, coords, uniform_buffer.target_texture_size);
    let rgb = srgb_from_yuv(yuv, uniform_buffer.yuv_matrix_coefficients, uniform_buffer.yuv_range);

    return vec4f(rgb, 1.0);
}
