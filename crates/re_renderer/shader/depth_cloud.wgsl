//! Renders a point cloud from a depth texture and a set of intrinsics.
//!
//! See `src/renderer/depth_cloud.rs` for more documentation.

#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/camera.wgsl>
#import <./utils/flags.wgsl>
#import <./utils/quad.wgsl>
#import <./utils/size.wgsl>

// ---

struct PointData {
    pos_in_world: Vec3,
    unresolved_radius: f32,
    color: Vec4
}

// Backprojects the depth texture using the intrinsics passed in the uniform buffer.
fn compute_point_data(quad_idx: i32) -> PointData {
    let wh = textureDimensions(depth_texture);
    let texcoords = IVec2(quad_idx % wh.x, quad_idx / wh.x);

    // TODO(cmc): expose knobs to linearize/normalize/flip/cam-to-plane depth.
    let norm_linear_depth = textureLoad(depth_texture, texcoords, 0).x;

    // TODO(cmc): support color maps & albedo textures
    let d = pow(norm_linear_depth, 2.2);
    let color = Vec4(d, d, d, 1.0);

    // TODO(cmc): This assumes a pinhole camera; need to support other kinds at some point.
    let intrinsics = transpose(depth_cloud_info.intrinsics);
    let focal_length = Vec2(intrinsics[0][0], intrinsics[1][1]);
    let offset = Vec2(intrinsics[2][0], intrinsics[2][1]);

    let pos_in_obj = Vec3(
        (Vec2(texcoords) - offset) * norm_linear_depth / focal_length,
        norm_linear_depth,
    );

    let pos_in_world = depth_cloud_info.world_from_obj * Vec4(pos_in_obj, 1.0);

    var data: PointData;
    data.pos_in_world = pos_in_world.xyz;
    data.unresolved_radius = norm_linear_depth * depth_cloud_info.radius_scale;
    data.color = color;

    return data;
}

// ---

struct DepthCloudInfo {
    world_from_obj: Mat4,

    /// The intrinsics of the camera used for the projection.
    ///
    /// Only supports pinhole cameras at the moment.
    intrinsics: Mat3,

    /// The scale to apply to the radii of the backprojected points.
    radius_scale: f32,
};
@group(1) @binding(0)
var<uniform> depth_cloud_info: DepthCloudInfo;

@group(1) @binding(1)
var depth_texture: texture_2d<f32>;

struct VertexOut {
    @builtin(position) pos_in_clip: Vec4,
    @location(0) pos_in_world: Vec3,
    @location(1) point_pos_in_world: Vec3,
    @location(2) point_color: Vec4,
    @location(3) point_radius: f32,
};

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
    let radius = unresolved_size_to_world(point_data.unresolved_radius, camera_distance, frame.auto_size_points);

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
    let d = ray_sphere_distance(ray_in_world, in.point_pos_in_world, in.point_radius);
    let smallest_distance_to_sphere = d.x;
    let closest_ray_dist = d.y;
    let pixel_world_size = approx_pixel_world_size_at(closest_ray_dist);
    if smallest_distance_to_sphere > pixel_world_size {
        discard;
    }

    // NOTE: We only want clipping, alpha coverage and shading don't look great for this use case.

    // TODO(andreas): Do we want manipulate the depth buffer depth to actually render spheres?

    return vec4(in.point_color.rgb, 1.0);
}
