#import <./types.wgsl>
#import <./screen_triangle_vertex.wgsl>

var<private> positions: array<vec2f, 3> = array<vec2f, 3>(
    vec2f(-1.0, -3.0),
    vec2f(-1.0, 1.0),
    vec2f(3.0, 1.0)
);

@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4f(positions[vertex_index], 0.0, 1.0);
    out.texcoord = out.position.xy * 0.5 + 0.5;
    out.texcoord.y = 1.0 - out.texcoord.y;
    return out;
}
