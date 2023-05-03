#import <../global_bindings.wgsl>
#import <../types.wgsl>
#import <./size.wgsl>

/// Span a quad in a way that guarantees that we'll be able to draw a perspective correct sphere
/// on it.
fn sphere_quad(
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

    // Add half a pixel of margin for the feathering we do for antialiasing the spheres.
    // It's fairly subtle but if we don't do this our spheres look slightly squarish
    modified_radius += 0.5 * frame.pixel_world_size_from_camera_distance * camera_distance;

    return point_pos + pos_in_quad * modified_radius + camera_offset * quad_normal;

    // normal billboard (spheres are cut off!):
    //      pos = point_data.pos + pos_in_quad * point_radius;
    // only enlarged billboard (works but requires z care even for non-overlapping spheres):
    //      modified_radius = length(toCamera) * radius / sqrt(distance_to_camera_sq - radius_sq);
    //      pos = particleCenter + quadPosition * modified_radius;
}

/// Span a quad in a way that guarantees that we'll be able to draw an orthographic correct sphere
/// on it.
fn circle_quad(point_pos: Vec3, point_radius: f32, top_bottom: f32, left_right: f32) -> Vec3 {
    let quad_normal = frame.camera_forward;
    let quad_right = normalize(cross(quad_normal, frame.view_from_world[1].xyz)); // It's spheres so any orthogonal vector would do.
    let quad_up = cross(quad_right, quad_normal);
    let pos_in_quad = top_bottom * quad_up + left_right * quad_right;

    // Add half a pixel of margin for the feathering we do for antialiasing the spheres.
    // It's fairly subtle but if we don't do this our spheres look slightly squarish
    // TODO(andreas): Computing distance to camera here is a bit excessive, should get distance more easily - keep in mind this code runs for ortho & perspective.
    let radius = point_radius + 0.5 * approx_pixel_world_size_at(distance(point_pos, frame.camera_position));

    return point_pos + pos_in_quad * radius;
}

/// Returns the index of the current quad.
fn sphere_quad_index(vertex_idx: u32) -> u32 {
    return vertex_idx / 6u;
}

struct SphereQuadData {
    pos_in_world: Vec3,
    point_resolved_radius: f32,
}

/// Span a quad onto which circles or perspective correct spheres can be drawn.
///
/// Note that in orthographic mode, spheres are always circles.
fn sphere_or_circle_quad_span(vertex_idx: u32, point_pos: Vec3, point_unresolved_radius: f32,
                                radius_boost_in_ui_points: f32, force_circle: bool) -> SphereQuadData {
    // Resolve radius to a world size. We need the camera distance for this, which is useful later on.
    let to_camera = frame.camera_position - point_pos;
    let camera_distance = length(to_camera);
    let radius = unresolved_size_to_world(point_unresolved_radius, camera_distance, frame.auto_size_points) +
                 world_size_from_point_size(radius_boost_in_ui_points, camera_distance);

    // Basic properties of the vertex we're at.
    let local_idx = vertex_idx % 6u;
    let top_bottom = f32(local_idx <= 1u || local_idx == 5u) * 2.0 - 1.0; // 1 for a top vertex, -1 for a bottom vertex.
    let left_right = f32(vertex_idx % 2u) * 2.0 - 1.0; // 1 for a right vertex, -1 for a left vertex.

    // Span quad
    var pos: Vec3;
    if is_camera_orthographic() || force_circle {
        pos = circle_quad(point_pos, radius, top_bottom, left_right);
    } else {
        pos = sphere_quad(point_pos, radius, top_bottom, left_right, to_camera, camera_distance);
    }

    return SphereQuadData(pos, radius);
}

/// Computes coverage of a 3D sphere placed at `sphere_center` in the fragment shader using the currently set camera.
fn sphere_quad_coverage(world_position: Vec3, radius: f32, sphere_center: Vec3) -> f32 {
    let ray = camera_ray_to_world_pos(world_position);

    // Sphere intersection with anti-aliasing as described by Iq here
    // https://www.shadertoy.com/view/MsSSWV
    // (but rearranged and labeled to it's easier to understand!)
    let d = ray_sphere_distance(ray, sphere_center, radius);
    let distance_to_sphere_surface = d.x;
    let closest_ray_dist = d.y;
    let pixel_world_size = approx_pixel_world_size_at(closest_ray_dist);

    let distance_to_surface_in_pixels = distance_to_sphere_surface / pixel_world_size;

    // At the surface we have 50% coverage, and it decreases with distance.
    // Note that we have signed distances to the sphere surface.
    return saturate(0.5 - distance_to_surface_in_pixels);
}
