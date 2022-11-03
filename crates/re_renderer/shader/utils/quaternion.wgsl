fn quat_rotate_vec3(q: Vec4, v: Vec3) -> Vec3 {
    // via glam's quaternion.rs
    let b = q.xyz;
    return v * (q.w * q.w - dot(b, b)) +
          (b * (dot(v, b) * 2.0)) +
          (cross(b, v) * (q.w * 2.0));
}
