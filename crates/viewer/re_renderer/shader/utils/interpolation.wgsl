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
fn catmull_rom_weights(t: f32) -> vec4f {
    let t2 = t * t;
    let t3 = t2 * t;
    return 0.5 * vec4f(
        -t3 + 2.0 * t2 - t,        // weight for p[-1]
         3.0 * t3 - 5.0 * t2 + 2.0, // weight for p[0]
        -3.0 * t3 + 4.0 * t2 + t,   // weight for p[1]
         t3 - t2                     // weight for p[2]
    );
}
