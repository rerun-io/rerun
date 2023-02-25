#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/camera.wgsl>
#import <./utils/flags.wgsl>
#import <./utils/size.wgsl>

// ---

// TODO: move that to include

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

// Returns distance to sphere surface (x) and distance to of closest ray hit (y)
// Via https://iquilezles.org/articles/spherefunctions/ but with more verbose names.
fn sphere_distance(ray: Ray, sphere_origin: Vec3, sphere_radius: f32) -> Vec2 {
    let sphere_radius_sq = sphere_radius * sphere_radius;
    let sphere_to_origin = ray.origin - sphere_origin;
    let b = dot(sphere_to_origin, ray.direction);
    let c = dot(sphere_to_origin, sphere_to_origin) - sphere_radius_sq;
    let h = b * b - c;
    let d = sqrt(max(0.0, sphere_radius_sq - h)) - sphere_radius;
    return Vec2(d, -b - sqrt(max(h, 0.0)));
}

// ---

struct PointData {
    pos_in_world: Vec3,
    color: Vec4
}

// Compute point data from depth.
fn compute_point_data(quad_idx: i32) -> PointData {
    let texcoords = IVec2(
        quad_idx % textureDimensions(depth_texture).x,
        quad_idx / textureDimensions(depth_texture).x,
    );

    // TODO: deal with..:
    // - non-linear depths
    // - reversed depths
    // - depth to plane vs. depth to cam
    let linear_depth = textureLoad(depth_texture, texcoords, 0).x;

    // TODO: support color maps & albedo textures
    let color = Vec4(linear_depth, linear_depth, linear_depth, 1.0);

    let pos_in_model = Vec3(linear_depth); // TODO: compute from linear depth... somehow

    // TODO: all of this is prob a mess
    var data: PointData;
    data.pos_in_world = (depth_cloud_info.world_from_model * Vec4(pos_in_model, 1.0)).xyz;
    data.color = color;

    return data;
}


// ---

// TODO: shading_enabled, radius
struct DepthCloudInfo {
    world_from_model: Mat4,
    model_from_world: Mat4,
};
@group(1) @binding(0)
var<uniform> depth_cloud_info: DepthCloudInfo;

@group(1) @binding(1)
var depth_texture: texture_2d<f32>;

// @group(1) @binding(2)
// var albedo_texture: texture_2d<f32>;

struct VertexOut {
    @builtin(position) pos_in_clip: Vec4,
    @location(0) pos_in_world: Vec3,
    @location(1) point_pos_in_world: Vec3,
    @location(2) point_color: Vec4,
    @location(3) point_radius: f32,
};

// TODO: arbitrary model2world transforms + move cam into model space during raytracing

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {

    // Basic properties of the vertex we're at.
    let quad_idx = i32(vertex_index) / 6;
    let local_idx = vertex_index % 6u;
    let top_bottom = f32(local_idx <= 1u || local_idx == 5u) * 2.0 - 1.0; // 1 for a top vertex, -1 for a bottom vertex.
    let left_right = f32(vertex_index % 2u) * 2.0 - 1.0; // 1 for a right vertex, -1 for a left vertex.

    // Compute point data (valid for the entire quad).
    let point_data = compute_point_data(quad_idx);
    // Resolve radius to a world size. We need the camera distance for this, which is useful later on.
    let to_camera = frame.camera_position - point_data.pos_in_world;
    let camera_distance = length(to_camera);
    // TODO: no clue what I'm doing.
    let radius = unresolved_size_to_world(1.0, camera_distance, frame.auto_size_points);

    // Span quad
    var pos_in_world: Vec3;
    if is_camera_perspective() {
        pos_in_world = span_quad_perspective(point_data.pos_in_world, radius, top_bottom, left_right, to_camera, camera_distance);
    } else {
        pos_in_world = span_quad_orthographic(point_data.pos_in_world, radius, top_bottom, left_right);
    }

    var out: VertexOut;
    out.pos_in_clip = frame.projection_from_world * Vec4(pos_in_world, 1.0);
    out.pos_in_world = pos_in_world;
    out.point_pos_in_world = point_data.pos_in_world;
    out.point_color = point_data.color;
    out.point_radius = radius;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {
    // There's easier ways to compute anti-aliasing for when we are in ortho mode since it's
    // just circles.
    // But it's very nice to have mostly the same code path and this gives us the sphere world
    // position along the way.
    let ray_in_world = camera_ray_to_world_pos(in.pos_in_world);

    // Sphere intersection with anti-aliasing as described by Iq here
    // https://www.shadertoy.com/view/MsSSWV
    // (but rearranged and labled to it's easier to understand!)
    let d = sphere_distance(ray_in_world, in.point_pos_in_world, in.point_radius);
    let smallest_distance_to_sphere = d.x;
    let closest_ray_dist = d.y;
    let pixel_world_size = approx_pixel_world_size_at(closest_ray_dist);
    if smallest_distance_to_sphere > pixel_world_size {
        discard;
    }
    let coverage = 1.0 - saturate(smallest_distance_to_sphere / pixel_world_size);

    // TODO(andreas): Do we want manipulate the depth buffer depth to actually render spheres?

    // TODO: do we actually care about shading at all for this use case?
    // // TODO(andreas): Proper shading
    // // TODO(andreas): This doesn't even use the sphere's world position for shading, the world position used here is flat!
    var shading = 1.0;
    // shading = max(0.4, sqrt(1.2 - distance(in.point_center, in.world_position) / in.point_radius)); // quick and dirty coloring
    return vec4(in.point_color.rgb * shading, coverage);
}
