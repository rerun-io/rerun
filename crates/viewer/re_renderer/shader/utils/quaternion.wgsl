fn quat_rotate_vec3f(q: vec4f, v: vec3f) -> vec3f {
    // via glam's quaternion.rs
    let b = q.xyz;
    return v * (q.w * q.w - dot(b, b)) +
          (b * (dot(v, b) * 2.0)) +
          (cross(b, v) * (q.w * 2.0));
}
