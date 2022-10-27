// workaround for https://github.com/gfx-rs/naga/issues/2006
fn unpack4x8unorm_workaround(v: u32) -> vec4<f32> {
    let shifted = vec4<u32>(v, v >> 8u, v >> 16u, v >> 24u);
    let bytes = shifted & vec4<u32>(0xFFu);
    return vec4<f32>(bytes) * (1.0 / 255.0);
}
