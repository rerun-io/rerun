#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./mesh_vertex.wgsl>
#import <./utils/quaternion.wgsl>
#import <./utils/srgb.wgsl>

@group(1) @binding(0)
var albedo_texture: texture_2d<f32>;

struct VertexOut {
    @builtin(position) position: Vec4,
    @location(0) texcoord: Vec2,
    @location(1) normal_world_space: Vec3,
    @location(2) color_tint: Vec3,
};

@vertex
fn vs_main(in_vertex: VertexIn, in_instance: InstanceIn) -> VertexOut {
    let world_position = quat_rotate_vec3(in_instance.rotation, in_vertex.position * in_instance.position_and_scale.w) +
                         in_instance.position_and_scale.xyz;

    var out: VertexOut;
    out.position = frame.projection_from_world * Vec4(world_position, 1.0);
    out.texcoord = in_vertex.texcoord;
    out.normal_world_space = quat_rotate_vec3(in_instance.rotation, in_vertex.normal);
    out.color_tint = linear_from_srgb(in_instance.tint_color_srgb.rgb);

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {
    let albedo = textureSample(albedo_texture, trilinear_sampler, in.texcoord).rgb + in.color_tint;

    // Hardcoded lambert lighting. TODO(andreas): Some microfacet model.
    let light_dir = normalize(vec3(1.0, 2.0, 0.0)); // TODO(andreas): proper lighting
    let normal = normalize(in.normal_world_space);
    let shading = clamp(dot(normal, light_dir), 0.0, 1.0) + 0.2;

    let radiance = albedo * shading;

    return Vec4(radiance, 1.0);
}
