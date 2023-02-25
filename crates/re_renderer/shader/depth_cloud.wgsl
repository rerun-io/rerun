#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/camera.wgsl>
#import <./utils/flags.wgsl>
#import <./utils/quad.wgsl>
#import <./utils/size.wgsl>

// ---

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
    linear_depth: f32,
    color: Vec4
}

// Compute point data from depth.
fn compute_point_data(quad_idx: i32) -> PointData {
    let texcoords = IVec2(
        quad_idx % textureDimensions(depth_texture).x,
        textureDimensions(depth_texture).y - quad_idx / textureDimensions(depth_texture).x,
    );

    // TODO: deal with..:
    // - non-linear depths
    // - reversed depths
    // - depth to plane vs. depth to cam
    let linear_depth = textureLoad(depth_texture, texcoords, 0).x;

    // TODO(cmc): support color maps & albedo textures
    let d = pow(clamp(linear_depth, 0.01, 1.00), 2.2);
    let color = Vec4(d, d, d, 1.0);

    // TODO(cmc): This assumes a pinhole camera; need to support other kinds at some point.

    let uv_center = Vec2(textureDimensions(depth_texture)) * 0.5;
    let focal_length = 0.7 * f32(textureDimensions(depth_texture).x);

    let plane_distance = 7.0; // TODO
    let pos_in_model = Vec3(
        (f32(texcoords.x) - uv_center.x) * linear_depth / focal_length,
        (f32(texcoords.y) - uv_center.y) * linear_depth / focal_length,
        linear_depth,
    ) * plane_distance;

    var data: PointData;
    data.pos_in_world = pos_in_model.xyz;
    data.linear_depth = linear_depth;
    data.color = color;

    return data;
}

// ---

struct DepthCloudInfo {
    intrinsics: Mat3,
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
    let radius = unresolved_size_to_world(-point_data.linear_depth * 4.0, camera_distance, frame.auto_size_points);

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

    // TODO(andreas): Do we want manipulate the depth buffer depth to actually render spheres?

    return vec4(in.point_color.rgb, 1.0);
}
