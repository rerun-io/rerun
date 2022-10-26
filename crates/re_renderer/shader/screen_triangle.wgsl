#import <./types.wgsl>

struct VertexOutput {
    @builtin(position) position: Vec4,
    @location(0) texcoord: Vec2,
};

var<private> positions: array<Vec2, 3> = array<Vec2, 3>(
    Vec2(-1.0, -3.0),
    Vec2(-1.0, 1.0),
    Vec2(3.0, 1.0)
);

@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    out.position = Vec4(positions[vertex_index], 0.0, 1.0);
    out.texcoord = out.position.xy * 0.5 + 0.5;
    out.texcoord.y = 1.0 - out.texcoord.y;
    return out;
}
