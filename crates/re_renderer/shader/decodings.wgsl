#import <./types.wgsl>


/// Loads an RGBA texel from a texture holding an NV12 or YUY2 encoded image at the given screen space coordinates.
fn decode_nv12_or_yuy2(sample_type: u32, texture: texture_2d<u32>, coords: vec2i) -> vec4f {
    let texture_dim = vec2f(textureDimensions(texture).xy);
    var y: f32;
    var u: f32;
    var v: f32;

    // WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING!
    // NO MORE SAMPLE TYPES CAN BE ADDED TO THIS SHADER!
    // The shader is already too large and adding more sample types will push us over the size limit.
    // See: https://github.com/rerun-io/rerun/issues/3931, https://github.com/rerun-io/rerun/issues/5073
    if sample_type == SAMPLE_TYPE_NV12 {
        let uv_offset = u32(floor(texture_dim.y / 1.5));
        let uv_row = u32(coords.y / 2);
        var uv_col = u32(coords.x / 2) * 2u;

        y = f32(textureLoad(texture, vec2u(coords), 0).r);
        u = f32(textureLoad(texture, vec2u(u32(uv_col), uv_offset + uv_row), 0).r);
        v = f32(textureLoad(texture, vec2u((u32(uv_col) + 1u), uv_offset + uv_row), 0).r);
    } else if sample_type == SAMPLE_TYPE_YUY2 {
        // texture is 2 * width * height
        // every 4 bytes is 2 pixels
        let uv_row = u32(coords.y);
        // multiply by 2 because the width is multiplied by 2
        let y_col = u32(coords.x) * 2u;
        y = f32(textureLoad(texture, vec2u(y_col, uv_row), 0).r);

        // at odd pixels we're in the second half of the yuyu block, offset back by 2
        let uv_col = y_col - u32(coords.x % 2) * 2u;
        u = f32(textureLoad(texture, vec2u(uv_col + 1u, uv_row), 0).r);
        v = f32(textureLoad(texture, vec2u(uv_col + 3u, uv_row), 0).r);
    }
    // WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING! WARNING!

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
