#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./mesh_vertex.wgsl>
#import <./utils/srgb.wgsl>

@group(1) @binding(0)
var albedo_texture: texture_2d<f32>;

// Keep in sync with gpu_data::MaterialUniformBuffer in mesh.rs
struct MaterialUniformBuffer {
    albedo_factor: vec4f,
};

@group(1) @binding(1)
var<uniform> material: MaterialUniformBuffer;

struct VertexOut {
    @builtin(position)
    position: vec4f,

    @location(0)
    color: vec4f, // 0-1 linear space with unmultiplied/separate alpha

    @location(1)
    texcoord: vec2f,

    @location(2)
    normal_world_space: vec3f,

    @location(3) @interpolate(flat)
    additive_tint_rgb: vec3f, // 0-1 linear space

    @location(4) @interpolate(flat)
    outline_mask_ids: vec2u,

    @location(5) @interpolate(flat)
    picking_layer_id: vec4u,
};

@vertex
fn vs_main(in_vertex: VertexIn, in_instance: InstanceIn) -> VertexOut {
    let world_position = vec3f(
        dot(in_instance.world_from_mesh_row_0.xyz, in_vertex.position) + in_instance.world_from_mesh_row_0.w,
        dot(in_instance.world_from_mesh_row_1.xyz, in_vertex.position) + in_instance.world_from_mesh_row_1.w,
        dot(in_instance.world_from_mesh_row_2.xyz, in_vertex.position) + in_instance.world_from_mesh_row_2.w,
    );
    let world_normal = vec3f(
        dot(in_instance.world_from_mesh_normal_row_0.xyz, in_vertex.normal),
        dot(in_instance.world_from_mesh_normal_row_1.xyz, in_vertex.normal),
        dot(in_instance.world_from_mesh_normal_row_2.xyz, in_vertex.normal),
    );

    var out: VertexOut;
    out.position = frame.projection_from_world * vec4f(world_position, 1.0);
    out.color = linear_from_srgba(in_vertex.color);
    out.texcoord = in_vertex.texcoord;
    out.normal_world_space = world_normal;
    out.additive_tint_rgb = linear_from_srgb(in_instance.additive_tint_srgb.rgb);
    out.outline_mask_ids = in_instance.outline_mask_ids;
    out.picking_layer_id = in_instance.picking_layer_id;

    return out;
}

@fragment
fn fs_main_shaded(in: VertexOut) -> @location(0) vec4f {
    let albedo = textureSample(albedo_texture, trilinear_sampler, in.texcoord).rgb
                 * in.color.rgb
                 * material.albedo_factor.rgb
                 + in.additive_tint_rgb;

    if all(in.normal_world_space == vec3f(0.0, 0.0, 0.0)) {
        // no normal, no shading
        return vec4f(albedo, 1.0);
    } else {
        // Hardcoded lambert lighting. TODO(andreas): Some microfacet model.
        let light_dir = normalize(vec3f(1.0, 2.0, 0.0)); // TODO(andreas): proper lighting
        let normal = normalize(in.normal_world_space);
        let shading = clamp(dot(normal, light_dir), 0.0, 1.0) + 0.2;

        let radiance = albedo * shading;

        return vec4f(radiance, 1.0);
    }
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    return in.picking_layer_id;
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    return in.outline_mask_ids;
}
