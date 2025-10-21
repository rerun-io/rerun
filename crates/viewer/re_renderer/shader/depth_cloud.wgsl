//! Renders a point cloud from a depth texture and a set of intrinsics.
//!
//! See `src/renderer/depth_cloud.rs` for more documentation.

#import <./colormap.wgsl>
#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/camera.wgsl>
#import <./utils/flags.wgsl>
#import <./utils/size.wgsl>
#import <./utils/sphere_quad.wgsl>
#import <./utils/srgb.wgsl>

// ---

// Keep in sync with `DepthCloudInfoUBO` in `depth_cloud.rs`.

// Which texture to read from?
const SAMPLE_TYPE_FLOAT = 1u;
const SAMPLE_TYPE_SINT  = 2u;
const SAMPLE_TYPE_UINT  = 3u;

/// Same for all draw-phases.
struct DepthCloudInfo {
    /// The extrinsincs of the camera used for the projection.
    world_from_rdf: mat4x4f,

    /// The intrinsics of the camera used for the projection.
    ///
    /// Only supports pinhole cameras at the moment.
    depth_camera_intrinsics: mat3x3f,

    /// Outline mask id for the outline mask pass.
    outline_mask_id: vec2u,

    /// Picking object id that applies for the entire depth cloud.
    picking_layer_object_id: vec2u,

    /// Multiplier to get world-space depth from whatever is in the texture.
    world_depth_from_texture_value: f32,

    /// Point radius is calculated as world-space depth times this value.
    point_radius_from_world_depth: f32,

    /// The minimum & maximum depth value in world-space, for use with the colormap.
    min_max_depth_in_world: vec2f,

    /// Configures color mapping mode, see `colormap.wgsl`.
    colormap: u32,

    /// Which texture sample to use
    sample_type: u32,

    /// Changes between the opaque and outline draw-phases.
    radius_boost_in_ui_points: f32,
};

@group(1) @binding(0)
var<uniform> depth_cloud_info: DepthCloudInfo;

@group(1) @binding(1)
var texture_float: texture_2d<f32>;

@group(1) @binding(2)
var texture_sint: texture_2d<i32>;

@group(1) @binding(3)
var texture_uint: texture_2d<u32>;

struct VertexOut {
    @builtin(position)
    pos_in_clip: vec4f,

    @location(0) @interpolate(perspective)
    pos_in_world: vec3f,

    @location(1) @interpolate(flat)
    point_pos_in_world: vec3f,

    @location(2) @interpolate(flat)
    point_color: vec4f,

    @location(3) @interpolate(flat)
    point_radius: f32,

    @location(4) @interpolate(flat)
    quad_idx: u32,
};

// ---

struct PointData {
    pos_in_world: vec3f,
    unresolved_radius: f32,
    color: vec4f,
}

// Backprojects the depth texture using the intrinsics passed in the uniform buffer.
fn compute_point_data(quad_idx: u32) -> PointData {
    var texcoords: vec2u;
    var texture_value = 0.0;
    if depth_cloud_info.sample_type == SAMPLE_TYPE_FLOAT {
        let wh = textureDimensions(texture_float);
        texcoords = vec2u(quad_idx % wh.x, quad_idx / wh.x);
        texture_value = textureLoad(texture_float, texcoords, 0).x;
    } else if depth_cloud_info.sample_type == SAMPLE_TYPE_SINT {
        let wh = textureDimensions(texture_sint);
        texcoords = vec2u(quad_idx % wh.x, quad_idx / wh.x);
        texture_value = f32(textureLoad(texture_sint, texcoords, 0).x);
    } else {
        let wh = textureDimensions(texture_uint);
        texcoords = vec2u(quad_idx % wh.x, quad_idx / wh.x);
        texture_value = f32(textureLoad(texture_uint, texcoords, 0).x);
    }

    // TODO(cmc): expose knobs to linearize/normalize/flip/cam-to-plane depth.
    let world_space_depth = depth_cloud_info.world_depth_from_texture_value * texture_value;

    var data: PointData;

    if 0.0 < world_space_depth && world_space_depth < f32max {
        // TODO(cmc): albedo textures
        let normalized_depth =
            (world_space_depth - depth_cloud_info.min_max_depth_in_world.x) /
            (depth_cloud_info.min_max_depth_in_world.y - depth_cloud_info.min_max_depth_in_world.x);
        let color = vec4f(colormap_linear(depth_cloud_info.colormap, normalized_depth), 1.0);

        // TODO(cmc): This assumes a pinhole camera; need to support other kinds at some point.
        let intrinsics = depth_cloud_info.depth_camera_intrinsics;
        let focal_length = vec2f(intrinsics[0][0], intrinsics[1][1]);
        let offset = vec2f(intrinsics[2][0], intrinsics[2][1]);

        // RDF: X=Right, Y=Down, Z=Forward
        let pos_in_rdf = vec3f(
            (vec2f(texcoords) - offset) * world_space_depth / focal_length,
            world_space_depth, // RDF, Z=forward, so positive depth
        );

        let pos_in_world = depth_cloud_info.world_from_rdf * vec4f(pos_in_rdf, 1.0);

        data.pos_in_world = pos_in_world.xyz;
        data.unresolved_radius = depth_cloud_info.point_radius_from_world_depth * world_space_depth;
        data.color = color;
    } else {
        // Degenerate case
        data.pos_in_world = vec3f(0.0);
        data.unresolved_radius = 0.0;
        data.color = vec4f(0.0);
    }
    return data;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    let quad_idx = sphere_quad_index(vertex_idx);

    // Compute point data (valid for the entire quad).
    let point_data = compute_point_data(quad_idx);

    var out: VertexOut;
    out.point_pos_in_world = point_data.pos_in_world;
    out.point_color = point_data.color;
    out.quad_idx = u32(quad_idx);

    if 0.0 < point_data.unresolved_radius {
        // Span quad
        let camera_distance = distance(frame.camera_position, point_data.pos_in_world);
        let world_scale_factor = average_scale_from_transform(depth_cloud_info.world_from_rdf); // TODO(andreas): somewhat costly, should precompute this
        let world_radius = unresolved_size_to_world(point_data.unresolved_radius, camera_distance, world_scale_factor) +
                           world_size_from_point_size(depth_cloud_info.radius_boost_in_ui_points, camera_distance);
        let quad = sphere_or_circle_quad_span(vertex_idx, point_data.pos_in_world, world_radius, false);
        out.pos_in_clip = frame.projection_from_world * vec4f(quad.pos_in_world, 1.0);
        out.pos_in_world = quad.pos_in_world;
        out.point_radius = quad.point_resolved_radius;
    } else {
        // Degenerate case - early-out!
        out.pos_in_clip = vec4f(0.0);
        out.pos_in_world = vec3f(0.0);
        out.point_radius = 0.0;
    }

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    var coverage = sphere_quad_coverage(in.pos_in_world, in.point_radius, in.point_pos_in_world);

    if frame.deterministic_rendering == 1 {
        coverage = step(0.5, coverage);
    }

    if coverage < 0.001 {
        discard;
    }
    return vec4f(in.point_color.rgb, coverage);
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    let coverage = sphere_quad_coverage(in.pos_in_world, in.point_radius, in.point_pos_in_world);
    if coverage <= 0.5 {
        discard;
    }
    return vec4u(depth_cloud_info.picking_layer_object_id, in.quad_idx, 0u);
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    // Output is an integer target so we can't use coverage even though
    // the target is anti-aliased.
    let coverage = sphere_quad_coverage(in.pos_in_world, in.point_radius, in.point_pos_in_world);
    if coverage <= 0.5 {
        discard;
    }
    return depth_cloud_info.outline_mask_id;
}
