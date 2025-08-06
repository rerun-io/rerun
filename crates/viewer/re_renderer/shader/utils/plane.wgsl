// A plane defined by `dot(normal, X) - distance = 0`.
//
// This is known as Hesse normal form.
// Note that we're subtracting distance, in some literature this term is added instead.
// This form is more usually more convenient as it expresses "above the plane" as `dot(normal, X) > distance`.
struct Plane {
    normal: vec3f,
    distance: f32,
}

/// How far away is a given point from the plane?
///
/// Returns a *signed* distance! Positive, when the point is above the plane, negative when below.
fn distance_to_plane(plane: Plane, point: vec3f) -> f32 {
    return dot(plane.normal, point) - plane.distance;
}
