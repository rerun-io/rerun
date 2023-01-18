#import <../global_bindings.wgsl>
#import <camera.wgsl>



fn unresolved_size_to_world(_unresolved_size: f32, camera_distance: f32, auto_size: f32) -> f32 {
    // Resolve auto size.
    var unresolved_size: f32;
    if _unresolved_size == inf() {
        // positive inf for small auto size
        unresolved_size = auto_size;
    } else if _unresolved_size == -inf() {
        // negative inf for large auto size
        let large_factor = 2.0;
        unresolved_size = auto_size * large_factor;
    } else {
        unresolved_size = _unresolved_size;
    }

    // Is it a world size?
    if unresolved_size > 0.0 {
        return unresolved_size;
    }

    // Negative size indicates size in points.
    let pixel_size = frame.pixels_from_point * (-unresolved_size);
    return approx_pixel_world_size_at(camera_distance) * pixel_size;
}
