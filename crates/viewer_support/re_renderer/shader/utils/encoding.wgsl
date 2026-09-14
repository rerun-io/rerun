// workaround for https://github.com/gfx-rs/naga/issues/2006
fn unpack4x8unorm_workaround(v: u32) -> vec4f {
    let shifted = vec4u(v, v >> 8u, v >> 16u, v >> 24u);
    let bytes = shifted & vec4u(0xFFu);
    return vec4f(bytes) * (1.0 / 255.0);
}
