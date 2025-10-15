#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/camera.wgsl>
#import <./utils/flags.wgsl>
#import <./utils/size.wgsl>
#import <./utils/sphere_quad.wgsl>
#import <./utils/depth_offset.wgsl>

@group(1) @binding(0)
var position_data_texture: texture_2d<f32>;
@group(1) @binding(1)
var color_texture: texture_2d<f32>;
@group(1) @binding(2)
var picking_instance_id_texture: texture_2d<u32>;

struct DrawDataUniformBuffer {
    radius_boost_in_ui_points: f32,
    // In actuality there is way more padding than this since we align all our uniform buffers to
    // 256bytes in order to allow them to be buffer-suballocations.
    // However, wgpu doesn't know this at this point and therefore requires `DownlevelFlags::BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED`
    // if we wouldn't add padding here, which isn't available on WebGL.
    _padding: vec4f,
};
@group(1) @binding(3)
var<uniform> draw_data: DrawDataUniformBuffer;

struct BatchUniformBuffer {
    world_from_obj: mat4x4f,
    flags: u32,
    depth_offset: f32,
    _padding: vec2u,
    outline_mask: vec2u,
    picking_layer_object_id: vec2u,
};
@group(2) @binding(0)
var<uniform> batch: BatchUniformBuffer;

// Flags
// See point_cloud.rs#PointCloudBatchFlags
const FLAG_ENABLE_SHADING: u32 = 1u;
const FLAG_DRAW_AS_CIRCLES: u32 = 2u;

struct VertexOut {
    @builtin(position)
    position: vec4f,

    @location(0) @interpolate(perspective)
    world_position: vec3f,

    @location(1) @interpolate(flat)
    radius: f32,

    @location(2) @interpolate(flat)
    point_center: vec3f,

    // TODO(andreas): Color & picking layer instance are only used in some passes.
    // Once we have shader variant support we should remove the unused ones
    // (it's unclear how good shader compilers are at removing unused outputs and associated texture fetches)
    // TODO(andreas): Is fetching color & picking layer in the fragment shader maybe more efficient?
    // Yes, that's more fetches but all of these would be cache hits whereas vertex data pass through can be expensive, (especially on tiler architectures!)

    @location(3) @interpolate(flat)
    color: vec4f, // linear RGBA with unmulitplied/separate alpha

    @location(4) @interpolate(flat)
    picking_instance_id: vec2u,
};

struct PointData {
    pos: vec3f,
    unresolved_radius: f32,
    color: vec4f,
    picking_instance_id: vec2u,
}

// Read and unpack data at a given location
fn read_data(idx: u32) -> PointData {
    let position_data_texture_size = textureDimensions(position_data_texture);
    let position_data = textureLoad(position_data_texture,
         vec2u(idx % position_data_texture_size.x, idx / position_data_texture_size.x), 0);

    let color_texture_size = textureDimensions(color_texture);
    let color = textureLoad(color_texture,
         vec2u(idx % color_texture_size.x, idx / color_texture_size.x), 0);

    let picking_instance_id_texture_size = textureDimensions(picking_instance_id_texture);
    let picking_instance_id = textureLoad(picking_instance_id_texture,
         vec2u(idx % picking_instance_id_texture_size.x, idx / picking_instance_id_texture_size.x), 0).xy;

    var data: PointData;
    let pos_4d = batch.world_from_obj * vec4f(position_data.xyz, 1.0);
    data.pos = pos_4d.xyz / pos_4d.w;
    data.unresolved_radius = position_data.w;
    data.color = color;
    data.picking_instance_id = picking_instance_id;
    return data;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    let quad_idx = sphere_quad_index(vertex_idx);

    // Read point data (valid for the entire quad)
    let point_data = read_data(quad_idx);

    // Span quad
    let camera_distance = distance(frame.camera_position, point_data.pos);
    let world_scale_factor = average_scale_from_transform(batch.world_from_obj); // TODO(andreas): somewhat costly, should precompute this
    let world_radius = unresolved_size_to_world(point_data.unresolved_radius, camera_distance, world_scale_factor) +
                       world_size_from_point_size(draw_data.radius_boost_in_ui_points, camera_distance);
    let quad = sphere_or_circle_quad_span(vertex_idx, point_data.pos, world_radius,
                                             has_any_flag(batch.flags, FLAG_DRAW_AS_CIRCLES));

    // Output, transform to projection space and done.
    var out: VertexOut;
    out.position = apply_depth_offset(frame.projection_from_world * vec4f(quad.pos_in_world, 1.0), batch.depth_offset);
    out.color = point_data.color;
    out.radius = quad.point_resolved_radius;
    out.world_position = quad.pos_in_world;
    out.point_center = point_data.pos;
    out.picking_instance_id = point_data.picking_instance_id;

    return out;
}

// TODO(andreas): move this to sphere_quad.wgsl once https://github.com/gfx-rs/naga/issues/1743 is resolved
// point_cloud.rs has a specific workaround in place so we don't need to split vertex/fragment shader here
//
/// Computes coverage of a 2D sphere placed at `circle_center` in the fragment shader using the currently set camera.
///
/// 2D primitives are always facing the camera - the difference to sphere_quad_coverage is that
/// perspective projection is not taken into account.
fn circle_quad_coverage(world_position: vec3f, radius: f32, circle_center: vec3f) -> f32 {
    let circle_distance = distance(circle_center, world_position);
    let feathering_radius = fwidth(circle_distance) * 0.5;
    return smoothstep(radius + feathering_radius, radius - feathering_radius, circle_distance);
}

fn coverage(world_position: vec3f, radius: f32, point_center: vec3f) -> f32 {
    if is_camera_orthographic() || has_any_flag(batch.flags, FLAG_DRAW_AS_CIRCLES) {
        return circle_quad_coverage(world_position, radius, point_center);
    } else {
        return sphere_quad_coverage(world_position, radius, point_center);
    }
}


@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    var coverage = coverage(in.world_position, in.radius, in.point_center);

    if frame.deterministic_rendering == 1 {
        coverage = step(0.5, coverage);
    }

    if coverage < 0.001 {
        discard;
    }

    // TODO(andreas): Do we want manipulate the depth buffer depth to actually render spheres?
    // TODO(andreas): Proper shading
    // TODO(andreas): This doesn't even use the sphere's world position for shading, the world position used here is flat!
    var shading = 1.0;
    if has_any_flag(batch.flags, FLAG_ENABLE_SHADING) {
        shading = max(0.4, sqrt(1.2 - distance(in.point_center, in.world_position) / in.radius)); // quick and dirty coloring
    }
    return vec4f(in.color.rgb * shading, coverage);
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    let cov = coverage(in.world_position, in.radius, in.point_center);
    if cov <= 0.5 {
        discard;
    }
    return vec4u(batch.picking_layer_object_id, in.picking_instance_id);
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    // Output is an integer target so we can't use coverage even though
    // the target is anti-aliased.
    let cov = coverage(in.world_position, in.radius, in.point_center);
    if cov <= 0.5 {
        discard;
    }
    return batch.outline_mask;
}
