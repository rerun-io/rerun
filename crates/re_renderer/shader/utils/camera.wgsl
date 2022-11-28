fn is_camera_orthographic() -> bool {
    return frame.tan_half_fov.x == 1.0 / 0.0;
}

fn is_camera_perspective() -> bool {
    return frame.tan_half_fov.x != 1.0 / 0.0;
}
