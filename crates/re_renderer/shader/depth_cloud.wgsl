//! Renders a point cloud from a depth texture and a set of intrinsics.
//!
//! See `src/renderer/depth_cloud.rs` for more documentation.

#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/camera.wgsl>
#import <./utils/flags.wgsl>
#import <./utils/size.wgsl>
#import <./utils/sphere_quad.wgsl>
#import <./utils/srgb.wgsl>

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
    let color = Vec4(linear_from_srgb(Vec3(norm_linear_depth)), 1.0);

    // TODO(cmc): This assumes a pinhole camera; need to support other kinds at some point.
    let intrinsics = transpose(depth_cloud_info.depth_camera_intrinsics);
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
    depth_camera_intrinsics: Mat3,

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
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    let quad_idx = sphere_quad_index(vertex_idx);

    // Compute point data (valid for the entire quad).
    let point_data = compute_point_data(quad_idx);

    // Span quad
    let quad = sphere_quad_span(vertex_idx, point_data.pos_in_world, point_data.unresolved_radius);

    var out: VertexOut;
    out.pos_in_clip = frame.projection_from_world * Vec4(quad.pos_in_world, 1.0);
    out.pos_in_world = quad.pos_in_world;
    out.point_pos_in_world = point_data.pos_in_world;
    out.point_color = point_data.color;
    out.point_radius = quad.point_resolved_radius;

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
    // (but rearranged and labeled to it's easier to understand!)
    let d = ray_sphere_distance(ray_in_world, in.point_pos_in_world, in.point_radius);
    let smallest_distance_to_sphere = d.x;
    let closest_ray_dist = d.y;
    let pixel_world_size = approx_pixel_world_size_at(closest_ray_dist);
    if smallest_distance_to_sphere > pixel_world_size {
        discard;
    }
    let coverage = 1.0 - saturate(smallest_distance_to_sphere / pixel_world_size);

    return vec4(in.point_color.rgb, coverage);
}
