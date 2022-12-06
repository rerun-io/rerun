fn size_to_world(unresolved_size: f32, default_pixel_size: f32, camera_distance: f32) -> f32 {
    if unresolved_size > 0.0 {
        // It's already a world size.
        return unresolved_size;
    }

    var pixel_size: f32;
    if unresolved_size != unresolved_size {
        // NaN indicates automatic/default size.
        pixel_size = default_pixel_size;
    } else {
        // Negative indicates size in points.
        pixel_size = frame.pixels_from_point * -unresolved_size;
    }

    return pixel_world_size_at(camera_distance) * pixel_size;
}
