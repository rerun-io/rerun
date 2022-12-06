// TODO(andreas): Lot of assumed includes here. need pragma once behavior.

fn unresolved_size_to_world(unresolved_size: f32, camera_distance: f32) -> f32 {
    var point_size: f32;
    if unresolved_size == inf() {
        // positive inf for small auto size
        point_size = frame.auto_size_in_points;
    } else if unresolved_size > 0.0 {
        // It's already a world size.
        return unresolved_size;
    } else if unresolved_size == -inf() {
        // negative inf for small auto size
        point_size = frame.auto_size_large_in_points;
    } else {
        // Negative size indicates size in points.
        point_size = -unresolved_size;
    }
    let pixel_size = frame.pixels_from_point * point_size;

    return approx_pixel_world_size_at(camera_distance) * pixel_size;
}
