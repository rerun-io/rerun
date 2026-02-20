//! Additional interpolation functions.

// Like smoothstep, but linear. 1D.
fn linearstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    return saturate((x - edge0) / (edge1 - edge0));
}

// Like smoothstep, but linear. 2D.
fn linearstep2(edge0: vec2f, edge1: vec2f, x: vec2f) -> vec2f {
    return saturate((x - edge0) / (edge1 - edge0));
}

/// Compute Catmull-Rom spline weights for 4 control points.
/// `t` is the fractional position between the two center points (0..1).
/// Returns weights for points at positions -1, 0, 1, 2 relative to the integer coordinate.
fn catmull_rom_weights(t: f32) -> vec4<f32> {
    let t2 = t * t;
    let t3 = t2 * t;

    let w_m1 = -t3 + 2.0 * t2 - t;
    let w_0  =  3.0 * t3 - 5.0 * t2 + 2.0;
    let w_1  = -3.0 * t3 + 4.0 * t2 + t;
    let w_2  =  t3 - t2;

    return 0.5 * vec4<f32>(w_m1, w_0, w_1, w_2);
}
