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

// Transforms a view-space vector to world-space.
// Note that since view_from_world is an orthonormal matrix, multiplying it from the right
// means multiplying it with the transpose, meaning multiplying with the inverse!
// (i.e. we get world_from_view for free as long as we only care about directions!)
fn view_space_to_world_space(view_space: vec3f) -> vec3f {
    return (view_space * frame.view_from_world).xyz;
}

// Converts screen UV coordinates (0 to 1) to Normalized Device Coordinates (-1 to +1)
fn screenuv_to_ndc(texcoord: vec2f) -> vec2f {
    return vec2f(texcoord.x - 0.5, 0.5 - texcoord.y) * 2.0;
}

// Converts pixel coordinates to Normalized Device Coordinates (-1 to +1)
fn fragcoord_to_ndc(fragcoord: vec2f) -> vec2f {
    let texcoord = fragcoord / frame.framebuffer_resolution.xy;
    return screenuv_to_ndc(texcoord);
}

// Returns the camera ray direction through a given screen uv coordinates (ranging from 0 to 1, i.e. NOT ndc coordinates)
fn camera_ray_direction_from_screenuv(texcoord: vec2f) -> vec3f {
    if is_camera_orthographic() {
        return frame.camera_forward;
    }

    let ndc = screenuv_to_ndc(texcoord);

    // Negative z since z dir is towards viewer (by current RUB convention).
    let view_space_dir = vec3f(ndc * frame.tan_half_fov, -1.0);

    return normalize(view_space_to_world_space(view_space_dir));
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
    ray.direction = camera_ray_direction_from_fragcoord(fragcoord);

    if is_camera_orthographic() {
        // For orthographic cameras, rays originate on the camera plane
        let ndc = fragcoord_to_ndc(fragcoord);

        // Compute viewport extent in world space
        let viewport_world_size = frame.pixel_world_size_from_camera_distance * frame.framebuffer_resolution;
        let view_space_offset = vec3f(ndc * viewport_world_size * 0.5, 0.0);

        // Offset the ray origin based on the fragment position in the camera plane
        ray.origin = frame.camera_position + view_space_to_world_space(view_space_offset);
    } else {
        // For perspective cameras, all rays originate from the camera position
        ray.origin = frame.camera_position;
    }

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
