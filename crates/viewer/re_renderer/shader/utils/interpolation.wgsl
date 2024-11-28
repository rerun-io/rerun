//! Additional interpolation functions.

// Like smoothstep, but linear. 1D.
fn linearstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    return saturate((x - edge0) / (edge1 - edge0));
}

// Like smoothstep, but linear. 2D.
fn linearstep2(edge0: vec2f, edge1: vec2f, x: vec2f) -> vec2f {
    return saturate((x - edge0) / (edge1 - edge0));
}
