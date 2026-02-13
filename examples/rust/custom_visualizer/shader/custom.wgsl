#import <types.wgsl>
#import <global_bindings.wgsl>

struct UniformBuffer {
    world_from_obj: mat4x4f,

    color: vec4f,

    picking_layer_object_id: vec2u,
    picking_instance_id: vec2u,

    outline_mask: vec2u,
};
@group(1) @binding(0)
var<uniform> ubo: UniformBuffer;

struct VertexOut {
    @location(0) color: vec4f,
    @builtin(position) position: vec4f,
};

var<private> v_positions: array<vec2f, 3> = array<vec2f, 3>(
    vec2f(0.0, 1.0),
    vec2f(1.0, -1.0),
    vec2f(-1.0, -1.0),
);

var<private> v_colors: array<vec4f, 3> = array<vec4f, 3>(
    vec4f(1.0, 0.0, 0.0, 1.0),
    vec4f(0.0, 1.0, 0.0, 1.0),
    vec4f(0.0, 0.0, 1.0, 1.0),
);

@vertex
fn vs_main(@builtin(vertex_index) v_idx: u32) -> VertexOut {
    var out: VertexOut;

    let position_obj =  vec4f(v_positions[v_idx], 0.0, 1.0);
    let position_world = ubo.world_from_obj * position_obj;
    let position_clip = frame.projection_from_world * position_world;

    out.position = position_clip;
    out.color = v_colors[v_idx] * ubo.color;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    return in.color;
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    return vec4u(ubo.picking_layer_object_id, ubo.picking_instance_id);
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    return ubo.outline_mask;
}
