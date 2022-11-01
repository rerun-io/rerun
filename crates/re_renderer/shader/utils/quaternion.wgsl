fn quat_rotate_vec3(q: Vec4, v: Vec3) -> Vec3 {
    // via glam quaternion.rs
    let b = q.xyz;
    let b2 = dot(b, b);
    return v * (q.w * q.w - b2) +
          (b * (dot(v, b) * 2.0)) +
          (cross(b, v) * (q.w * 2.0));
}
