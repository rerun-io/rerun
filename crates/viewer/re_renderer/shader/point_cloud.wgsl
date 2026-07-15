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
    // Index of this batch's first point in the shared point-data textures.
    // This is effectively the min instance index!
    first_point_index: u32,
    _padding: u32,
    outline_mask: vec2u,
    picking_layer_object_id: vec2u,
};
@group(2) @binding(0)
var<uniform> batch: BatchUniformBuffer;

@group(3) @binding(0)
var point_index_lookup_texture: texture_2d<u32>;

// Flags
// See point_cloud.rs#PointCloudBatchFlags
const FLAG_ENABLE_SHADING: u32 = 1u;
const FLAG_DRAW_AS_CIRCLES: u32 = 2u;
const FLAG_PREMULTIPLIED_ALPHA: u32 = 4u;
const FLAG_ENABLE_INDEX_LOOKUP: u32 = 8u;

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

    // Offset vector from `point_center` to the quad.
    // Interpolating along this small-scale local offset for coverage math avoids float-precision issues,
    // compared to subtracting potentially large world positions in the fragment shader.
    @location(5) @interpolate(perspective)
    quad_offset_from_center: vec3f,
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
    var point_idx = quad_idx;
    if has_any_flag(batch.flags, FLAG_ENABLE_INDEX_LOOKUP) {
        let lookup_idx = quad_idx - batch.first_point_index;
        let lookup_texture_size = textureDimensions(point_index_lookup_texture);
        point_idx = batch.first_point_index + textureLoad(
            point_index_lookup_texture,
            vec2u(lookup_idx % lookup_texture_size.x, lookup_idx / lookup_texture_size.x),
            0,
        ).x;
    }

    // Read point data (valid for the entire quad)
    let point_data = read_data(point_idx);

    // Span quad
    // The pixel size formula `pixel_world_size_from_camera_distance` is derived from
    // `screen_in_world = tan(FOV/2) * depth * 2` so it expects a distance along the
    // camera forward axis.
    let camera_distance = dot(point_data.pos - frame.camera_position, frame.camera_forward);
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
    out.quad_offset_from_center = quad.pos_in_world - point_data.pos;
    out.picking_instance_id = point_data.picking_instance_id;

    return out;
}

fn coverage(world_position: vec3f, radius: f32, point_center: vec3f, quad_offset_from_center: vec3f) -> f32 {
    if is_camera_orthographic() || has_any_flag(batch.flags, FLAG_DRAW_AS_CIRCLES) {
        return circle_quad_coverage(quad_offset_from_center, radius);
    } else {
        return sphere_quad_coverage(world_position, radius, point_center);
    }
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    var coverage = coverage(
        in.world_position,
        in.radius,
        in.point_center,
        in.quad_offset_from_center,
    );

    if frame.deterministic_rendering == 1 {
        coverage = step(0.5, coverage);
    }

    // As per benchmarking on Apple Silicon M5, putting a discard can be
    // a significant pessimization in high-overdraw situations and at best only a very mild optimization.
    // Desktop graphics cards like the tested RTX4070 do not exhibit this behavior and aren't affected by it at all.
    // This is likely due to Apple Silicon being a tile based GPU, it's still a bit surprising though int he presence of alpha-to-coverage.
    // (Disabling alpha-to-coverage also shows performance benefits _independently_ of the discard instructionß)
    // if coverage < 0.001 {
    //     discard;
    // }

    // TODO(andreas): Do we want manipulate the depth buffer depth to actually render spheres?
    // TODO(andreas): Proper shading
    // TODO(andreas): This doesn't even use the sphere's world position for shading, the world position used here is flat!
    var shading = 1.0;
    if has_any_flag(batch.flags, FLAG_ENABLE_SHADING) {
        shading = max(0.4, sqrt(1.2 - distance(in.point_center, in.world_position) / in.radius)); // quick and dirty coloring
    }
    if has_any_flag(batch.flags, FLAG_PREMULTIPLIED_ALPHA) {
        // Premultiplied alpha output for the no-alpha-to-coverage (alpha-blended) pipeline.
        return vec4f(in.color.rgb * shading, in.color.a) * coverage;
    } else {
        // Default alpha-to-coverage output: alpha encodes per-fragment coverage.
        return vec4f(in.color.rgb * shading, coverage);
    }
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    let cov = coverage(
        in.world_position,
        in.radius,
        in.point_center,
        in.quad_offset_from_center,
    );
    if cov <= 0.5 {
        discard;
    }
    return vec4u(batch.picking_layer_object_id, in.picking_instance_id);
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    // Output is an integer target so we can't use coverage even though
    // the target is anti-aliased.
    let cov = coverage(
        in.world_position,
        in.radius,
        in.point_center,
        in.quad_offset_from_center,
    );
    if cov <= 0.5 {
        discard;
    }
    return batch.outline_mask;
}
