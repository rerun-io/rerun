#import <../global_bindings.wgsl>
#import <camera.wgsl>


fn world_size_from_point_size(size_in_points: f32, camera_distance: f32) -> f32 {
    let pixel_size = frame.pixels_from_point * size_in_points;
    return approx_pixel_world_size_at(camera_distance) * pixel_size;
}

// Resolves a size (see size.rs!) to a world scale size.
//
// world_size_scale:
//      Scale factor that is applied iff the size is a world size.
//      This is usually part of your object->world transform.
fn unresolved_size_to_world(_unresolved_size: f32, camera_distance: f32, auto_size: f32, world_size_scale: f32) -> f32 {
    // Resolve auto size.
    var unresolved_size: f32;
    if _unresolved_size >= f32max {
        // positive max for small auto size
        unresolved_size = auto_size;
    } else if _unresolved_size <= f32min {
        // negative max for large auto size
        let large_factor = 1.33;
        unresolved_size = auto_size * large_factor;
    } else {
        unresolved_size = _unresolved_size;
    }

    // Is it a world size?
    if unresolved_size > 0.0 {
        return unresolved_size * world_size_scale;
    }

    // Negative size indicates size in points.
    return world_size_from_point_size(-unresolved_size, camera_distance);
}

// Determines the scale factor of a matrix
//
// This quite expensive, you may want to precompute this.
fn average_scale_from_transform(transform: mat4x4f) -> f32 {
    // Source: https://math.stackexchange.com/a/1463487
    // Won't work with negative scale.
    // Note we're only look at the scale, not at shear
    let scale = vec3f(length(transform[0].xyz), length(transform[1].xyz), length(transform[2].xyz));
    // Get geometric mean
    return pow(scale.x * scale.y * scale.z, 0.3333);
}
