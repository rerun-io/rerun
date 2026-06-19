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
};

struct UniformBuffer {
    world_from_grid: mat4x4f,
    voxel_size: vec3f,
    depth_offset: f32,
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
    instance_id: vec2u,
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

const CULLED_POSITION: vec4f = vec4f(2.0, 2.0, 2.0, 1.0);

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32, @builtin(instance_index) instance_idx: u32, in_instance: InstanceIn) -> VertexOut {
    var out: VertexOut;
    out.color = linear_from_srgba_premultiplied(in_instance.color_srgba);
    if out.color.a <= 0.0 {
        out.position = CULLED_POSITION;
        return out;
    }

    let cube_vertex_idx = vertex_idx % 36u;
    let grid_position = (vec3f(in_instance.index) + CUBE_VERTICES[cube_vertex_idx]) * batch.voxel_size;
    let world_position = batch.world_from_grid * vec4f(grid_position, 1.0);
    let normal_world_space = normalize((batch.world_from_grid * vec4f(CUBE_NORMALS[cube_vertex_idx / 6u], 0.0)).xyz);

    out.position = apply_depth_offset(frame.projection_from_world * world_position, batch.depth_offset);
    out.normal_world_space = normal_world_space;
    out.normal_world_space = vec3f(0.0, 0.0, 1.0);
    out.instance_id = vec2u(instance_idx, 0u);

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    let shading = simple_lighting(in.normal_world_space);
    return vec4f(in.color.rgb * shading * in.color.a, in.color.a);
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    return vec4u(batch.picking_layer_object_id, in.instance_id);
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    return batch.outline_mask;
}
