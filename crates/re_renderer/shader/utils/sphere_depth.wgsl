#import <../global_bindings.wgsl>

// Routines for computing per-fragment depth of billboarded spheres or cylinders.
//
// Note: This technique is only an approximation. It ignores that depth is non-linear, so the
// curve of the sphere's surface will be distorted.
//
// It also ignores the fact that spheres not in line with the camera's optical axis are
// distorted by perspective projection in an asymmetric way — their outlines may be ellipses,
// but the peak of the screen-space depth is *not* centered on the screen-space center of the
// ellipse. In addition to that lack of distortion, the `projected_with_offset` point is the
// nearest to the camera in world space, but is not the point with the nearest depth in screen
// space, so the sphere's depth is lower than it should be.
//
// To get it right, we'd need to, essentially, compute the ray-tracing of a sphere.
// But, overall, this produces spheres that have roughly consistent depth behavior independent of
// view direction, which is good enough to, for example, make the look of intersections of
// points and lines consistent as the camera orbits them. Just don't look too closely.


// Compute the maximum `frag_depth` offset that a sphere, of radius `object_radius`
// with center located at `object_position`, should have on the middle of its surface.
// This may be used for cylinders too by giving the position of the nearest point on its
// center line.
fn sphere_radius_projected_depth(object_position: vec3f, object_radius: f32) -> f32 {
    // If the billboard were a round mesh, where would the point on its surface nearest the camera be?
    let front_point = object_position
        + normalize(frame.camera_position - object_position) * object_radius;

    // Project that point and the object's center point.
    let projected_without_offset = frame.projection_from_world * vec4f(object_position, 1.0);
    let projected_with_offset = frame.projection_from_world * vec4f(front_point, 1.0);

    // Take the difference of the projected values.
    return projected_with_offset.z / projected_with_offset.w
         - projected_without_offset.z / projected_without_offset.w;
}

// Given the value computed by `sphere_radius_projected_depth()`, and the world-space position
// of the current fragment, compute what should be added to its `frag_depth` to produce the
// (approximate) depth value of the sphere.
fn sphere_fragment_projected_depth(
    object_radius: f32,
    sphere_radius_projected_depth: f32,
    world_frag_offset_from_center: vec3f,
) -> f32 {
    let offset_radius_squared =
        dot(world_frag_offset_from_center, world_frag_offset_from_center);
    let normalized_radius_squared = offset_radius_squared / (object_radius * object_radius);

    return sphere_radius_projected_depth * sqrt(clamp(1.0 - normalized_radius_squared, 0.0, 1.0));
}
