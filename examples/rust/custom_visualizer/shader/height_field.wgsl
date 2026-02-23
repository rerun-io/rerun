#import <types.wgsl>
#import <global_bindings.wgsl>
#import <colormap.wgsl>
#import <utils/lighting.wgsl>

struct UniformBuffer {
    world_from_obj: mat4x4f,

    grid_cols: u32,
    grid_rows: u32,
    spacing: f32,
    colormap: u32,

    min_height: f32,
    max_height: f32,
    _pad0: f32,
    _pad1: f32,

    picking_layer_object_id: vec2u,
    picking_instance_id: vec2u,

    outline_mask: vec2u,
};

@group(1) @binding(0)
var<uniform> ubo: UniformBuffer;

// ---------------------------------------------------------------------------

struct VertexIn {
    /// Height value from the vertex buffer (one f32 per grid vertex).
    @location(0) height: f32,

    /// Index from the index buffer, used to derive grid row/col.
    @builtin(vertex_index) v_idx: u32,
};

struct VertexOut {
    @location(0) color: vec4f,
    @location(1) position_world: vec3f,
    @builtin(position) position: vec4f,
};

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;

    // v_idx is a grid-linear index from the index buffer.
    let row = in.v_idx / ubo.grid_cols;
    let col = in.v_idx % ubo.grid_cols;

    // Flip Y so that image row 0 is at the top (large Y), matching image convention.
    let position_obj = vec4f(
        f32(col) * ubo.spacing,
        f32(ubo.grid_rows - 1u - row) * ubo.spacing,
        in.height,
        1.0,
    );

    let position_world = ubo.world_from_obj * position_obj;
    out.position = frame.projection_from_world * position_world;
    out.position_world = position_world.xyz;

    // Normalize height to [0,1] and apply the colormap.
    let height_range = ubo.max_height - ubo.min_height;
    let t = select((in.height - ubo.min_height) / height_range, 0.5, height_range <= 0.0);
    let color_rgb = colormap_linear(ubo.colormap, t);
    out.color = vec4f(color_rgb, 1.0);

    return out;
}

// ---------------------------------------------------------------------------
// Fragment shaders.
//
// Normals are computed from screen-space derivatives of the interpolated
// world position, so no neighbor access or extra vertex attributes are needed.

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    let normal = normalize(cross(dpdx(in.position_world), dpdy(in.position_world)));
    let shading = simple_lighting(normal);
    return vec4f(in.color.rgb * shading, in.color.a);
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    return vec4u(ubo.picking_layer_object_id, ubo.picking_instance_id);
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    return ubo.outline_mask;
}
