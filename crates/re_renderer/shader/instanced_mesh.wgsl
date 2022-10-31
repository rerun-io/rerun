#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./mesh_vertex.wgsl>

struct VertexOut {
    @builtin(position) position: Vec4,
    @location(0) texcoord: Vec2,
    @location(1) normal_obj_space: Vec3,
};

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;

    out.position = frame.projection_from_world * Vec4(in.position, 1.0);
    out.texcoord = in.texcoord;
    out.normal_obj_space = in.normal;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {

    let light_dir = normalize(vec3(1.0, 2.0, 0.0)); // TODO(andreas): proper lighting
    let normal = normalize(in.normal_obj_space);
    let shading = clamp(dot(normal, light_dir), 0.0, 1.0) + 0.2;

    return Vec4(shading, shading, shading, 0.0);
}
