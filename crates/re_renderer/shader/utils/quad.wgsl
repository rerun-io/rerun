#import <./global_bindings.wgsl>
#import <./types.wgsl>

fn span_quad_perspective(
    point_pos: Vec3,
    point_radius: f32,
    top_bottom: f32,
    left_right: f32,
    to_camera: Vec3,
    camera_distance: f32
) -> Vec3 {
    let distance_to_camera_sq = camera_distance * camera_distance; // (passing on micro-optimization here for splitting this out of earlier length calculation)
    let distance_to_camera_inv = 1.0 / camera_distance;
    let quad_normal = to_camera * distance_to_camera_inv;
    let quad_right = normalize(cross(quad_normal, frame.view_from_world[1].xyz)); // It's spheres so any orthogonal vector would do.
    let quad_up = cross(quad_right, quad_normal);
    let pos_in_quad = top_bottom * quad_up + left_right * quad_right;

    // But we want to draw pretend-spheres here!
    // If camera gets close to a sphere (or the sphere is large) then outlines of the sphere would not fit on a quad with radius r!
    // Enlarging the quad is one solution, but then Z gets tricky (== we need to write correct Z and not quad Z to depth buffer) since we may get
    // "unnecessary" overlaps. So instead, we change the size _and_ move the sphere closer (using math!)
    let radius_sq = point_radius * point_radius;
    let camera_offset = radius_sq * distance_to_camera_inv;
    var modified_radius = point_radius * distance_to_camera_inv * sqrt(distance_to_camera_sq - radius_sq);

    // We're computing a coverage mask in the fragment shader - make sure the quad doesn't cut off our antialiasing.
    // It's fairly subtle but if we don't do this our spheres look slightly squarish
    modified_radius += frame.pixel_world_size_from_camera_distance * camera_distance;

    return point_pos + pos_in_quad * modified_radius + camera_offset * quad_normal;

    // normal billboard (spheres are cut off!):
    //      pos = point_data.pos + pos_in_quad * point_radius;
    // only enlarged billboard (works but requires z care even for non-overlapping spheres):
    //      modified_radius = length(toCamera) * radius / sqrt(distance_to_camera_sq - radius_sq);
    //      pos = particleCenter + quadPosition * modified_radius;
}

fn span_quad_orthographic(point_pos: Vec3, point_radius: f32, top_bottom: f32, left_right: f32) -> Vec3 {
    let quad_normal = frame.camera_forward;
    let quad_right = normalize(cross(quad_normal, frame.view_from_world[1].xyz)); // It's spheres so any orthogonal vector would do.
    let quad_up = cross(quad_right, quad_normal);
    let pos_in_quad = top_bottom * quad_up + left_right * quad_right;

    // We're computing a coverage mask in the fragment shader - make sure the quad doesn't cut off our antialiasing.
    // It's fairly subtle but if we don't do this our spheres look slightly squarish
    let radius = point_radius + frame.pixel_world_size_from_camera_distance;

    return point_pos + pos_in_quad * radius;
}

