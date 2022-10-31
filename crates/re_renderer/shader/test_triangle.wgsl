#import <./types.wgsl>
#import <./global_bindings.wgsl>

struct VertexOut {
    @location(0) color: Vec4,
    @builtin(position) position: Vec4,
};

var<private> v_positions: array<Vec2, 3> = array<Vec2, 3>(
    Vec2(0.0, 10.0),
    Vec2(10.0, -10.0),
    Vec2(-10.0, -10.0),
);

// kek
var<private> v_colors: array<Vec4, 3> = array<Vec4, 3>(
    Vec4(1.0, 0.0, 0.0, 1.0),
    Vec4(0.0, 1.0, 0.0, 1.0),
    Vec4(0.0, 0.0, 1.0, 1.0),
);

@vertex
fn vs_main(@builtin(vertex_index) v_idx: u32) -> VertexOut {
    var out: VertexOut;

    out.position = frame.projection_from_world * Vec4(v_positions[v_idx], 0.0, 1.0);
    out.color = v_colors[v_idx];

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {
    return in.color;
}
