#import <./types.wgsl>


/// Loads an RGBA texel from a texture holding an NV12 encoded image at the given screen space coordinates.
fn decode_nv12(texture: texture_2d<u32>, coords: IVec2) -> Vec4 {
    let texture_dim = Vec2(textureDimensions(texture).xy);
    let uv_offset = u32(floor(texture_dim.y / 1.5));
    let uv_row = u32(coords.y / 2);
    var uv_col = u32(coords.x / 2) * 2u;

    let y = (f32(textureLoad(texture, UVec2(coords), 0).r) - 16.0) / 219.0;
    let u = (f32(textureLoad(texture, UVec2(u32(uv_col), uv_offset + uv_row), 0).r) - 128.0) / 224.0;
    let v = (f32(textureLoad(texture, UVec2((u32(uv_col) + 1u), uv_offset + uv_row), 0).r) - 128.0) / 224.0;

    let r = y + 1.402 * v;
    let g = y  - (0.344 * u + 0.714 * v);
    let b = y + 1.772 * u;
    return Vec4(r, g, b, 1.0);
}
