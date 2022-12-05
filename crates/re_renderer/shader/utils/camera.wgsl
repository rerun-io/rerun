// TODO(andreas): global_bindings are imported implicitely

fn inf() -> f32 {
    return 1.0 / 0.0;
}

// True if the camera is orthographic
fn is_camera_orthographic() -> bool {
    return frame.tan_half_fov.x == inf();
}

// True if the camera is perspective
fn is_camera_perspective() -> bool {
    return frame.tan_half_fov.x != inf();
}

struct Ray {
    origin: Vec3,
    direction: Vec3,
}

// Returns the ray from the camera to a given world position.
fn camera_ray_to_world_pos(world_pos: Vec3) -> Ray {
    var ray: Ray;

    if is_camera_perspective() {
        ray.origin = frame.camera_position;
        ray.direction = normalize(world_pos - frame.camera_position);
    } else {
        // The ray originates on the camera plane, not from the camera position
        let to_pos = world_pos - frame.camera_position;
        let camera_plane_distance = dot(to_pos, frame.camera_forward);
        ray.origin = world_pos - frame.camera_forward * camera_plane_distance;
        ray.direction = frame.camera_forward;
    }

    return ray;
}

// Returns the camera ray direction through a given screen uv coordinates (ranging from 0 to 1, i.e. NOT ndc coordinates)
fn camera_ray_direction_from_screenuv(texcoord: Vec2) -> vec3<f32> {
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

// Returns the projected size of a pixel at a given distance from the camera.
fn pixel_world_size_at(camera_distance: f32) -> f32 {
    return select(frame.pixel_world_size_from_camera_distance,
        camera_distance * frame.pixel_world_size_from_camera_distance, is_camera_perspective());
}
