#import <camera.wgsl>

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

/// Computes the intersection between a ray and a plane.
///
/// Returns the distance `t` along the ray to the intersection point.
/// The intersection point can be computed as: ray.origin + t * ray.direction
///
/// Note: Returns infinity if ray is parallel to plane, negative values if intersection is behind ray origin.
fn intersect_ray_plane(ray: Ray, plane: Plane) -> f32 {
    let denom = dot(plane.normal, ray.direction);
    return (plane.distance - dot(plane.normal, ray.origin)) / denom;
}

