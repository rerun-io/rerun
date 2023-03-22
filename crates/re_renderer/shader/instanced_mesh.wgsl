#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./mesh_vertex.wgsl>
#import <./utils/srgb.wgsl>

@group(1) @binding(0)
var albedo_texture: texture_2d<f32>;

// Keep in sync with gpu_data::MaterialUniformBuffer in mesh.rs
struct MaterialUniformBuffer {
    albedo_factor: Vec4,
};

@group(1) @binding(1)
var<uniform> material: MaterialUniformBuffer;

struct VertexOut {
    @builtin(position) position: Vec4,
    @location(0) color: Vec4, // 0-1 linear space with unmultiplied/separate alpha
    @location(1) texcoord: Vec2,
    @location(2) normal_world_space: Vec3,
    @location(3) additive_tint_rgb: Vec3,
    @location(4) @interpolate(flat)
    outline_mask_ids: UVec2,
};

@vertex
fn vs_main(in_vertex: VertexIn, in_instance: InstanceIn) -> VertexOut {
    let world_position = Vec3(
        dot(in_instance.world_from_mesh_row_0.xyz, in_vertex.position) + in_instance.world_from_mesh_row_0.w,
        dot(in_instance.world_from_mesh_row_1.xyz, in_vertex.position) + in_instance.world_from_mesh_row_1.w,
        dot(in_instance.world_from_mesh_row_2.xyz, in_vertex.position) + in_instance.world_from_mesh_row_2.w,
    );
    let world_normal = Vec3(
        dot(in_instance.world_from_mesh_normal_row_0.xyz, in_vertex.normal),
        dot(in_instance.world_from_mesh_normal_row_1.xyz, in_vertex.normal),
        dot(in_instance.world_from_mesh_normal_row_2.xyz, in_vertex.normal),
    );

    var out: VertexOut;
    out.position = frame.projection_from_world * Vec4(world_position, 1.0);
    out.color = linear_from_srgba(in_vertex.color);
    out.texcoord = in_vertex.texcoord;
    out.normal_world_space = world_normal;
    out.additive_tint_rgb = linear_from_srgb(in_instance.additive_tint_srgb.rgb);
    out.outline_mask_ids = in_instance.outline_mask_ids;

    return out;
}

@fragment
fn fs_main_shaded(in: VertexOut) -> @location(0) Vec4 {
    let albedo = textureSample(albedo_texture, trilinear_sampler, in.texcoord).rgb
                 * in.color.rgb
                 * material.albedo_factor.rgb
                 + in.additive_tint_rgb;

    if (all(in.normal_world_space == Vec3(0.0, 0.0, 0.0))) {
        // no normal, no shading
        return Vec4(albedo, 1.0);
    } else {
        // Hardcoded lambert lighting. TODO(andreas): Some microfacet model.
        let light_dir = normalize(vec3(1.0, 2.0, 0.0)); // TODO(andreas): proper lighting
        let normal = normalize(in.normal_world_space);
        let shading = clamp(dot(normal, light_dir), 0.0, 1.0) + 0.2;

        let radiance = albedo * shading;

        return Vec4(radiance, 1.0);
    }
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) UVec2 {
    return in.outline_mask_ids;
}
