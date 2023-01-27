// workaround for https://github.com/gfx-rs/naga/issues/2006
fn unpack4x8unorm_workaround(v: u32) -> Vec4 {
    let shifted = UVec4(v, v >> 8u, v >> 16u, v >> 24u);
    let bytes = shifted & UVec4(0xFFu);
    return Vec4(bytes) * (1.0 / 255.0);
}
