#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/depth_offset.wgsl>
#import <./utils/lighting.wgsl>
#import <./utils/srgb.wgsl>

struct InstanceIn {
    @location(0)
    index: vec3i,

    @location(1)
    color_srgba: vec4f,

    @location(2)
    picking_instance_id: vec2u,
};

struct UniformBuffer {
    world_from_grid: mat4x4f,
    cell_size: f32,
    opacity: f32,
    depth_offset: f32,
    _padding0: f32,
    picking_layer_object_id: vec2u,
    outline_mask: vec2u,
};

@group(1) @binding(0)
var<uniform> batch: UniformBuffer;

struct VertexOut {
    @builtin(position)
    position: vec4f,

    @location(0) @interpolate(flat)
    color: vec4f,

    @location(1) @interpolate(flat)
    normal_world_space: vec3f,

    @location(2) @interpolate(flat)
    picking_instance_id: vec2u,
};

const CUBE_VERTICES: array<vec3f, 36> = array<vec3f, 36>(
    // +X
    vec3f(1.0, 0.0, 0.0), vec3f(1.0, 1.0, 0.0), vec3f(1.0, 1.0, 1.0),
    vec3f(1.0, 0.0, 0.0), vec3f(1.0, 1.0, 1.0), vec3f(1.0, 0.0, 1.0),
    // -X
    vec3f(0.0, 0.0, 0.0), vec3f(0.0, 0.0, 1.0), vec3f(0.0, 1.0, 1.0),
    vec3f(0.0, 0.0, 0.0), vec3f(0.0, 1.0, 1.0), vec3f(0.0, 1.0, 0.0),
    // +Y
    vec3f(0.0, 1.0, 0.0), vec3f(0.0, 1.0, 1.0), vec3f(1.0, 1.0, 1.0),
    vec3f(0.0, 1.0, 0.0), vec3f(1.0, 1.0, 1.0), vec3f(1.0, 1.0, 0.0),
    // -Y
    vec3f(0.0, 0.0, 0.0), vec3f(1.0, 0.0, 0.0), vec3f(1.0, 0.0, 1.0),
    vec3f(0.0, 0.0, 0.0), vec3f(1.0, 0.0, 1.0), vec3f(0.0, 0.0, 1.0),
    // +Z
    vec3f(0.0, 0.0, 1.0), vec3f(1.0, 0.0, 1.0), vec3f(1.0, 1.0, 1.0),
    vec3f(0.0, 0.0, 1.0), vec3f(1.0, 1.0, 1.0), vec3f(0.0, 1.0, 1.0),
    // -Z
    vec3f(0.0, 0.0, 0.0), vec3f(0.0, 1.0, 0.0), vec3f(1.0, 1.0, 0.0),
    vec3f(0.0, 0.0, 0.0), vec3f(1.0, 1.0, 0.0), vec3f(1.0, 0.0, 0.0),
);

const CUBE_NORMALS: array<vec3f, 6> = array<vec3f, 6>(
    vec3f(1.0, 0.0, 0.0),
    vec3f(-1.0, 0.0, 0.0),
    vec3f(0.0, 1.0, 0.0),
    vec3f(0.0, -1.0, 0.0),
    vec3f(0.0, 0.0, 1.0),
    vec3f(0.0, 0.0, -1.0),
);

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32, in_instance: InstanceIn) -> VertexOut {
    let cube_vertex_idx = vertex_idx % 36u;
    let grid_position = (vec3f(in_instance.index) + CUBE_VERTICES[cube_vertex_idx]) * batch.cell_size;
    let world_position = batch.world_from_grid * vec4f(grid_position, 1.0);
    let normal_world_space = normalize((batch.world_from_grid * vec4f(CUBE_NORMALS[cube_vertex_idx / 6u], 0.0)).xyz);

    var out: VertexOut;
    out.position = apply_depth_offset(frame.projection_from_world * world_position, batch.depth_offset);
    out.color = vec4f(linear_from_srgb(in_instance.color_srgba.rgb), in_instance.color_srgba.a * batch.opacity);
    out.normal_world_space = normal_world_space;
    out.picking_instance_id = in_instance.picking_instance_id;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    if in.color.a <= 0.0 {
        discard;
    }

    let shading = simple_lighting(in.normal_world_space);
    return vec4f(in.color.rgb * shading * in.color.a, in.color.a);
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    if in.color.a <= 0.0 {
        discard;
    }

    return vec4u(batch.picking_layer_object_id, in.picking_instance_id);
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    if in.color.a <= 0.0 {
        discard;
    }

    return batch.outline_mask;
}
