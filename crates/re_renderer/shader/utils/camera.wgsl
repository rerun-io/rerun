// True if the camera is orthographic
fn is_camera_orthographic() -> bool {
    return frame.tan_half_fov.x == 1.0 / 0.0;
}

// True if the camera is perspective
fn is_camera_perspective() -> bool {
    return frame.tan_half_fov.x != 1.0 / 0.0;
}

struct Ray {
    origin: Vec3,
    direction: Vec3,
}

// Returns origin of a ray from the camera to a given position.
fn camera_ray_to_world_pos(world_pos: Vec3) -> Ray {
    var ray: Ray;

    if is_camera_perspective() {
        ray.origin = frame.camera_position;
        ray.direction = normalize(world_pos - frame.camera_position);
    } else {
        // The ray originates on the camera plane, not from the camera position
        let to_camera = frame.camera_position - world_pos;
        let camera_plane_distance = dot(to_camera, frame.camera_direction);
        ray.origin = world_pos + frame.camera_direction * camera_plane_distance;
        ray.direction = -frame.camera_direction;
    }

    return ray;
}
