#import <../global_bindings.wgsl>
#import <../types.wgsl>

// True if the camera is orthographic
fn is_camera_orthographic() -> bool {
    return frame.tan_half_fov.x >= f32max;
}

// True if the camera is perspective
fn is_camera_perspective() -> bool {
    return frame.tan_half_fov.x < f32max;
}

struct Ray {
    origin: vec3f,
    direction: vec3f,
}

// Returns the ray from the camera to a given world position, assuming the camera is perspective
fn camera_ray_to_world_pos_perspective(world_pos: vec3f) -> Ray {
    var ray: Ray;
    ray.origin = frame.camera_position;
    ray.direction = normalize(world_pos - frame.camera_position);
    return ray;
}

// Returns the ray from the camera to a given world position, assuming the camera is orthographic
fn camera_ray_to_world_pos_orthographic(world_pos: vec3f) -> Ray {
    var ray: Ray;
    // The ray originates on the camera plane, not from the camera position
    let to_pos = world_pos - frame.camera_position;
    let camera_plane_distance = dot(to_pos, frame.camera_forward);
    ray.origin = world_pos - frame.camera_forward * camera_plane_distance;
    ray.direction = frame.camera_forward;
    return ray;
}

// Returns the ray from the camera to a given world position.
fn camera_ray_to_world_pos(world_pos: vec3f) -> Ray {
    if is_camera_perspective() {
        return camera_ray_to_world_pos_perspective(world_pos);
    } else {
        return camera_ray_to_world_pos_orthographic(world_pos);
    }
}

// Returns the camera ray direction through a given screen uv coordinates (ranging from 0 to 1, i.e. NOT ndc coordinates)
fn camera_ray_direction_from_screenuv(texcoord: vec2f) -> vec3f {
    if is_camera_orthographic() {
        return frame.camera_forward;
    }

    // convert [0, 1] to [-1, +1 (Normalized Device Coordinates)
    let ndc = vec2f(texcoord.x - 0.5, 0.5 - texcoord.y) * 2.0;

    // Negative z since z dir is towards viewer (by current RUB convention).
    let view_space_dir = vec3f(ndc * frame.tan_half_fov, -1.0);

    // Note that since view_from_world is an orthonormal matrix, multiplying it from the right
    // means multiplying it with the transpose, meaning multiplying with the inverse!
    // (i.e. we get world_from_view for free as long as we only care about directions!)
    let world_space_dir = (view_space_dir * frame.view_from_world).xyz;

    return normalize(world_space_dir);
}

// Returns the camera ray direction through given pixel coordinates.
// (Assumes outputting to the framebuffer with resolution frame.framebuffer_resolution)
// fragcoord: pixel coordinates (e.g., from @builtin(position))
fn camera_ray_direction_from_fragcoord(fragcoord: vec2f) -> vec3f {
    let texcoord = fragcoord / frame.framebuffer_resolution.xy;
    return camera_ray_direction_from_screenuv(texcoord);
}

// Returns the camera ray through given pixel coordinates.
// (Assumes outputting to the framebuffer with resolution frame.framebuffer_resolution)
// fragcoord: pixel coordinates (e.g., from @builtin(position))
fn camera_ray_from_fragcoord(fragcoord: vec2f) -> Ray {
    var ray: Ray;
    ray.origin = frame.camera_position;
    ray.direction = camera_ray_direction_from_fragcoord(fragcoord);
    return ray;
}

// Returns distance to sphere surface (x) and distance to closest ray hit (y)
// Via https://iquilezles.org/articles/spherefunctions/ but with more verbose names.
fn ray_sphere_distance(ray: Ray, sphere_origin: vec3f, sphere_radius: f32) -> vec2f {
    let sphere_radius_sq = sphere_radius * sphere_radius;
    let sphere_to_origin = ray.origin - sphere_origin;
    let b = dot(sphere_to_origin, ray.direction);
    let c = dot(sphere_to_origin, sphere_to_origin) - sphere_radius_sq;
    let h = b * b - c;
    let d = sqrt(max(0.0, sphere_radius_sq - h)) - sphere_radius;
    return vec2f(d, -b - sqrt(max(h, 0.0)));
}

// Returns the projected size of a pixel at a given distance from the camera.
//
// This is accurate for objects in the middle of the screen, (depending on the angle) less so at the corners
// since an object parallel to the camera (like a conceptual pixel) has a bigger projected surface at higher angles.
fn approx_pixel_world_size_at(camera_distance: f32) -> f32 {
    return select(frame.pixel_world_size_from_camera_distance, camera_distance * frame.pixel_world_size_from_camera_distance, is_camera_perspective());
}
