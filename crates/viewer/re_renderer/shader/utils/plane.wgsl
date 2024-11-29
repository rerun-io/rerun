struct Plane {
    normal: vec3f,
    distance: f32,
}

/// How far away is a given point from the plane?
fn distance_to_plane(plane: Plane, point: vec3f) -> f32 {
    return dot(plane.normal, point) + plane.distance;
}
