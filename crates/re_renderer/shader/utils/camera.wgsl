#import <../global_bindings.wgsl>

// True if the camera is orthographic
fn is_camera_orthographic() -> bool {
    return frame.tan_half_fov.x >= f32max;
}

// True if the camera is perspective
fn is_camera_perspective() -> bool {
    return frame.tan_half_fov.x < f32max;
}

struct Ray {
    origin: Vec3,
    direction: Vec3,
}

// Returns the ray from the camera to a given world position, assuming the camera is perspective
fn camera_ray_to_world_pos_perspective(world_pos: Vec3) -> Ray {
    var ray: Ray;
    ray.origin = frame.camera_position;
    ray.direction = normalize(world_pos - frame.camera_position);
    return ray;
}

// Returns the ray from the camera to a given world position, assuming the camera is orthographic
fn camera_ray_to_world_pos_orthographic(world_pos: Vec3) -> Ray {
    var ray: Ray;
    // The ray originates on the camera plane, not from the camera position
    let to_pos = world_pos - frame.camera_position;
    let camera_plane_distance = dot(to_pos, frame.camera_forward);
    ray.origin = world_pos - frame.camera_forward * camera_plane_distance;
    ray.direction = frame.camera_forward;
    return ray;
}

// Returns the ray from the camera to a given world position.
fn camera_ray_to_world_pos(world_pos: Vec3) -> Ray {
    if is_camera_perspective() {
        return camera_ray_to_world_pos_perspective(world_pos);
    } else {
        return camera_ray_to_world_pos_orthographic(world_pos);
    }
}

// Returns the camera ray direction through a given screen uv coordinates (ranging from 0 to 1, i.e. NOT ndc coordinates)
fn camera_ray_direction_from_screenuv(texcoord: Vec2) -> Vec3 {
    if is_camera_orthographic() {
        return frame.camera_forward;
    }

    // convert [0, 1] to [-1, +1 (Normalized Device Coordinates)
    let ndc = Vec2(texcoord.x - 0.5, 0.5 - texcoord.y) * 2.0;

    // Negative z since z dir is towards viewer (by current RUB convention).
    let view_space_dir = Vec3(ndc * frame.tan_half_fov, -1.0);

    // Note that since view_from_world is an orthonormal matrix, multiplying it from the right
    // means multiplying it with the transpose, meaning multiplying with the inverse!
    // (i.e. we get world_from_view for free as long as we only care about directions!)
    let world_space_dir = (view_space_dir * frame.view_from_world).xyz;

    return normalize(world_space_dir);
}

// Returns distance to sphere surface (x) and distance to closest ray hit (y)
// Via https://iquilezles.org/articles/spherefunctions/ but with more verbose names.
fn ray_sphere_distance(ray: Ray, sphere_origin: Vec3, sphere_radius: f32) -> Vec2 {
    let sphere_radius_sq = sphere_radius * sphere_radius;
    let sphere_to_origin = ray.origin - sphere_origin;
    let b = dot(sphere_to_origin, ray.direction);
    let c = dot(sphere_to_origin, sphere_to_origin) - sphere_radius_sq;
    let h = b * b - c;
    let d = sqrt(max(0.0, sphere_radius_sq - h)) - sphere_radius;
    return Vec2(d, -b - sqrt(max(h, 0.0)));
}

// Returns the projected size of a pixel at a given distance from the camera.
//
// This is accurate for objects in the middle of the screen, (depending on the angle) less so at the corners
// since an object parallel to the camera (like a conceptual pixel) has a bigger projected surface at higher angles.
fn approx_pixel_world_size_at(camera_distance: f32) -> f32 {
    return select(frame.pixel_world_size_from_camera_distance, camera_distance * frame.pixel_world_size_from_camera_distance, is_camera_perspective());
}
