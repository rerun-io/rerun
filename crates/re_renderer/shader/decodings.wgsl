#import <./types.wgsl>


/// Loads an RGBA texel from a texture holding an NV12 encoded image at the given screen space coordinates.
fn decode_nv12(texture: texture_2d<u32>, coords: IVec2) -> Vec4 {
    let texture_dim = Vec2(textureDimensions(texture).xy);
    let uv_offset = u32(floor(texture_dim.y / 1.5));
    let uv_row = u32(coords.y / 2);
    var uv_col = u32(coords.x / 2) * 2u;

    let y = max(0.0, (f32(textureLoad(texture, UVec2(coords), 0).r) - 16.0)) / 219.0;
    let u = (f32(textureLoad(texture, UVec2(u32(uv_col), uv_offset + uv_row), 0).r) - 128.0) / 224.0;
    let v = (f32(textureLoad(texture, UVec2((u32(uv_col) + 1u), uv_offset + uv_row), 0).r) - 128.0) / 224.0;

    // BT.601 (aka. SDTV, aka. Rec.601). wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.601_conversion
    let r = clamp(y + 1.402 * v, 0.0, 1.0);
    let g = clamp(y  - (0.344 * u + 0.714 * v), 0.0, 1.0);
    let b = clamp(y + 1.772 * u, 0.0, 1.0);
    // BT.709 (aka. HDTV, aka. Rec.709). wiki: https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.709_conversion
    // let r = clamp(y + 1.5748 * v, 0.0, 1.0);
    // let g = clamp(y + u * -0.1873 + v * -0.4681, 0.0, 1.0);
    // let b = clamp(y + u * 1.8556, 0.0 , 1.0);
    return Vec4(r, g, b, 1.0);
}
