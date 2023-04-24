#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/camera.wgsl>
#import <./utils/flags.wgsl>
#import <./utils/size.wgsl>
#import <./utils/sphere_quad.wgsl>

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
    _padding: Vec4,
};
@group(1) @binding(3)
var<uniform> draw_data: DrawDataUniformBuffer;

struct BatchUniformBuffer {
    world_from_obj: Mat4,
    flags: u32,
    _padding: UVec2, // UVec3 would take its own 4xf32 row, UVec2 is on the same as flags
    outline_mask: UVec2,
    picking_layer_object_id: UVec2,
};
@group(2) @binding(0)
var<uniform> batch: BatchUniformBuffer;

// Flags
// See point_cloud.rs#PointCloudBatchFlags
const ENABLE_SHADING: u32 = 1u;
var<private> TEXTURE_SIZE: u32 = 2048;

struct VertexOut {
    @builtin(position)
    position: Vec4,

    @location(0) @interpolate(perspective)
    world_position: Vec3,

    @location(1) @interpolate(flat)
    radius: f32,

    @location(2) @interpolate(flat)
    point_center: Vec3,

    // TODO(andreas): Color & picking layer instance are only used in some passes.
    // Once we have shader variant support we should remove the unused ones
    // (it's unclear how good shader compilers are at removing unused outputs and associated texture fetches)
    // TODO(andreas): Is fetching color & picking layer in the fragment shader maybe more efficient?
    // Yes, that's more fetches but all of these would be cache hits whereas vertex data pass through can be expensive, (especially on tiler architectures!)

    @location(3) @interpolate(flat)
    color: Vec4, // linear RGBA with unmulitplied/separate alpha

    @location(4) @interpolate(flat)
    picking_instance_id: UVec2,
};

struct PointData {
    pos: Vec3,
    unresolved_radius: f32,
    color: Vec4,
    picking_instance_id: UVec2,
}

// Read and unpack data at a given location
fn read_data(idx: u32) -> PointData {
    let coord = UVec2(idx % TEXTURE_SIZE, idx / TEXTURE_SIZE);
    let position_data = textureLoad(position_data_texture, coord, 0);
    let color = textureLoad(color_texture, coord, 0);

    var data: PointData;
    let pos_4d = batch.world_from_obj * Vec4(position_data.xyz, 1.0);
    data.pos = pos_4d.xyz / pos_4d.w;
    data.unresolved_radius = position_data.w;
    data.color = color;
    data.picking_instance_id = textureLoad(picking_instance_id_texture, coord, 0).rg;
    return data;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    let quad_idx = sphere_quad_index(vertex_idx);

    // Read point data (valid for the entire quad)
    let point_data = read_data(quad_idx);

    // Span quad
    let quad = sphere_quad_span(vertex_idx, point_data.pos, point_data.unresolved_radius, draw_data.radius_boost_in_ui_points);

    // Output, transform to projection space and done.
    var out: VertexOut;
    out.position = frame.projection_from_world * Vec4(quad.pos_in_world, 1.0);
    out.color = point_data.color;
    out.radius = quad.point_resolved_radius;
    out.world_position = quad.pos_in_world;
    out.point_center = point_data.pos;
    out.picking_instance_id = point_data.picking_instance_id;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {
    let coverage = sphere_quad_coverage(in.world_position, in.radius, in.point_center);
    if coverage < 0.001 {
        discard;
    }

    // TODO(andreas): Do we want manipulate the depth buffer depth to actually render spheres?
    // TODO(andreas): Proper shading
    // TODO(andreas): This doesn't even use the sphere's world position for shading, the world position used here is flat!
    var shading = 1.0;
    if has_any_flag(batch.flags, ENABLE_SHADING) {
        shading = max(0.4, sqrt(1.2 - distance(in.point_center, in.world_position) / in.radius)); // quick and dirty coloring
    }
    return vec4(in.color.rgb * shading, coverage);
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) UVec4 {
    let coverage = sphere_quad_coverage(in.world_position, in.radius, in.point_center);
    if coverage <= 0.5 {
        discard;
    }
    return UVec4(batch.picking_layer_object_id, in.picking_instance_id);
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) UVec2 {
    // Output is an integer target, can't use coverage therefore.
    // But we still want to discard fragments where coverage is low.
    // Since the outline extends a bit, a very low cut off tends to look better.
    let coverage = sphere_quad_coverage(in.world_position, in.radius, in.point_center);
    if coverage < 1.0 {
        discard;
    }
    return batch.outline_mask;
}
