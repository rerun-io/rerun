#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./mesh_vertex.wgsl>

struct VertexOut {
    @builtin(position) position: Vec4,
    @location(0) texcoord: Vec2,
};

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;

    out.position = frame.projection_from_world * Vec4(in.position, 1.0);
    out.texcoord = in.texcoord;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {
    return Vec4(in.texcoord, 0.0, 0.0);
}
