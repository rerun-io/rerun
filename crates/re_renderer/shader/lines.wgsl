struct FrameUniformBuffer {
    view_from_world: mat4x3<f32>,
    projection_from_view: mat4x4<f32>,
    projection_from_world: mat4x4<f32>,

    camera_position: vec3<f32>,
    top_right_screen_corner_in_view: vec2<f32>,
};
@group(0) @binding(0)
var<uniform> frame: FrameUniformBuffer;

struct VertexOut {
    @location(0) color: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) v_idx: u32) -> VertexOut {
    var out: VertexOut;

    out.position = frame.projection_from_world * vec4<f32>(v_positions[v_idx], 0.0, 1.0);
    out.color = v_colors[v_idx];

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}
