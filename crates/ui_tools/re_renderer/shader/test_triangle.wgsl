#import <./types.wgsl>
#import <./global_bindings.wgsl>

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

    out.position = frame.projection_from_world * vec4f(v_positions[v_idx] * 5.0, 0.0, 1.0);
    out.color = v_colors[v_idx];

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    return in.color;
}
