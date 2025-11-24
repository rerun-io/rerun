#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/depth_offset.wgsl>

/// Uniform buffer for batch-level data.
struct BatchUniformBuffer {
    world_from_obj: mat4x4f,
    flags: u32,
    depth_offset: f32,
    _padding: vec2u,
    outline_mask: vec2u,
    picking_layer_object_id: vec2u,
};
@group(1) @binding(0)
var<uniform> batch: BatchUniformBuffer;

// Flags - keep in sync with box_cloud.rs#BoxCloudBatchFlags
const FLAG_ENABLE_SHADING: u32 = 1u;

/// Vertex attributes from the unit cube vertex buffer
struct VertexInput {
    @location(0) position: vec3f,    // Unit cube position [-0.5, 0.5]³
    @location(1) normal: vec3f,      // Face normal
};

/// Instance attributes from the instance buffer
struct InstanceInput {
    @location(2) center: vec3f,
    @location(3) half_size_x: f32,
    @location(4) half_size_yz: vec2f,
    @location(5) color: vec4f,
    @location(6) picking_instance_id: vec2u,
};

struct VertexOut {
    @builtin(position)
    position: vec4f,

    @location(0) @interpolate(perspective)
    world_position: vec3f,

    @location(1) @interpolate(flat)
    world_normal: vec3f,

    @location(2) @interpolate(flat)
    color: vec4f, // linear RGBA with unmultiplied/separate alpha

    @location(3) @interpolate(flat)
    picking_instance_id: vec2u,
};

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOut {
    // Reconstruct full half_size from split components
    let half_size = vec3f(instance.half_size_x, instance.half_size_yz.x, instance.half_size_yz.y);

    // Scale unit cube vertex by half_size (multiply by 2 because unit cube is [-0.5, 0.5])
    let local_pos = vertex.position * half_size * 2.0;

    // Translate to box center
    let box_position = instance.center + local_pos;

    // Apply world transform
    let world_pos_4d = batch.world_from_obj * vec4f(box_position, 1.0);
    let world_pos = world_pos_4d.xyz / world_pos_4d.w;

    // Transform normal (for axis-aligned scaling, just scale and normalize)
    let world_normal = normalize((batch.world_from_obj * vec4f(vertex.normal, 0.0)).xyz);

    // Output
    var out: VertexOut;
    out.position = apply_depth_offset(
        frame.projection_from_world * vec4f(world_pos, 1.0),
        batch.depth_offset
    );
    out.world_position = world_pos;
    out.world_normal = world_normal;
    // Color already provided in linear space.
    out.color = instance.color;
    out.picking_instance_id = instance.picking_instance_id;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    var shading = 1.0;

    if (batch.flags & FLAG_ENABLE_SHADING) != 0u {
        // Simple diffuse shading
        let normal = normalize(in.world_normal);

        // Headlight: light comes from camera
        let light_dir = normalize(frame.camera_position - in.world_position);

        // Lambertian diffuse with ambient
        let diffuse = max(dot(normal, light_dir), 0.0);
        let ambient = 0.4;
        shading = ambient + (1.0 - ambient) * diffuse;
    }

    // Apply shading to color
    return vec4f(in.color.rgb * shading, in.color.a);
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    return vec4u(batch.picking_layer_object_id, in.picking_instance_id);
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    return batch.outline_mask;
}
