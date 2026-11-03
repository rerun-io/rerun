fn quat_rotate_vec3f(q: vec4f, v: vec3f) -> vec3f {
    // via glam's quaternion.rs
    let b = q.xyz;
    return v * (q.w * q.w - dot(b, b)) +
          (b * (dot(v, b) * 2.0)) +
          (cross(b, v) * (q.w * 2.0));
}

// Rotation matrix from a (normalized) xyzw quaternion.
fn mat3_from_quat(q: vec4f) -> mat3x3f {
    let x2 = q.x + q.x;
    let y2 = q.y + q.y;
    let z2 = q.z + q.z;
    let xx = q.x * x2;
    let xy = q.x * y2;
    let xz = q.x * z2;
    let yy = q.y * y2;
    let yz = q.y * z2;
    let zz = q.z * z2;
    let wx = q.w * x2;
    let wy = q.w * y2;
    let wz = q.w * z2;
    return mat3x3f(
        vec3f(1.0 - (yy + zz), xy + wz, xz - wy),
        vec3f(xy - wz, 1.0 - (xx + zz), yz + wx),
        vec3f(xz + wy, yz - wx, 1.0 - (xx + yy)),
    );
}
