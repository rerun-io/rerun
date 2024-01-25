#import <./types.wgsl>


/// Loads an RGBA texel from a texture holding an NV12 encoded image at the given screen space coordinates.
fn decode_nv12(texture: texture_2d<u32>, coords: vec2i) -> vec4f {
    let texture_dim = vec2f(textureDimensions(texture).xy);
    let uv_offset = u32(floor(texture_dim.y / 1.5));
    let uv_row = u32(coords.y / 2);
    var uv_col = u32(coords.x / 2) * 2u;

    let y = f32(textureLoad(texture, vec2u(coords), 0).r);
    let u = f32(textureLoad(texture, vec2u(u32(uv_col), uv_offset + uv_row), 0).r);
    let v = f32(textureLoad(texture, vec2u((u32(uv_col) + 1u), uv_offset + uv_row), 0).r);

    let rgb = set_color_standard(vec3f(y, u, v));

    return vec4f(rgb, 1.0);
}

/// Loads an RGBA texel from a texture holding an YUY2 encoded image at the given screen space coordinates.
fn decode_yuy2(texture: texture_2d<u32>, coords: vec2i) -> vec4f {
    // texture is 2 * width * height
    // every 4 bytes is 2 pixels
    let uv_row = u32(coords.y);
    // multiply by 2 because the width is multiplied by 2
    var uv_col = u32(coords.x) * 2u;

    var y = 0.0;
    if coords.x % 2 == 0 {
        // we're on an even pixel, so we can sample the first y value
        y = f32(textureLoad(texture, vec2u(uv_col, uv_row), 0).r);
    } else {
        // we're on an odd pixel, so we need to sample the second y value
        // we add 2 to the column to get the second y value
        y = f32(textureLoad(texture, vec2u(uv_col + 2u, uv_row), 0).r);
        // We subtract 2 from the column so that we can sample the u and v values
        uv_col -= 2u;
    }
    let u = f32(textureLoad(texture, vec2u(uv_col + 1u, uv_row), 0).r);
    let v = f32(textureLoad(texture, vec2u(uv_col + 3u, uv_row), 0).r);

    let rgb = set_color_standard(vec3f(y, u, v));

    return vec4f(rgb, 1.0);
}

/// Sets the color standard for the given YUV color.
///
/// This conversion mirrors the function in `crates/re_types/src/datatypes/tensor_data_ext.rs`
///
/// Specifying the color standard should be exposed in the future [#3541](https://github.com/rerun-io/rerun/pull/3541)
fn set_color_standard(yuv: vec3f) -> vec3f {
    // rescale YUV values
    let y = (yuv.x - 16.0) / 219.0;
    let u = (yuv.y - 128.0) / 224.0;
    let v = (yuv.z - 128.0) / 224.0;

    // BT.601 (aka. SDTV, aka. Rec.601). wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.601_conversion
    let r = y + 1.402 * v;
    let g = y - 0.344 * u - 0.714 * v;
    let b = y + 1.772 * u;

    // BT.709 (aka. HDTV, aka. Rec.709). wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.709_conversion
    // let r = y + 1.575 * v;
    // let g = y - 0.187 * u - 0.468 * v;
    // let b = y + 1.856 * u;

    return vec3f(
        clamp(r, 0.0, 1.0),
        clamp(g, 0.0, 1.0),
        clamp(b, 0.0, 1.0)
    );
}
