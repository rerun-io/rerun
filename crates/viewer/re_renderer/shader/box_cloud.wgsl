#import <./global_bindings.wgsl>
#import <./types.wgsl>
#import <./utils/camera.wgsl>
#import <./utils/flags.wgsl>
#import <./utils/size.wgsl>
#import <./utils/box_quad.wgsl>
#import <./utils/depth_offset.wgsl>

@group(1) @binding(0)
var position_halfsize_texture: texture_2d<f32>;
@group(1) @binding(1)
var color_texture: texture_2d<f32>;
@group(1) @binding(2)
var picking_instance_id_texture: texture_2d<u32>;

struct DrawDataUniformBuffer {
    edge_radius_boost_in_ui_points: f32,
    // In actuality there is way more padding than this since we align all our uniform buffers to
    // 256bytes in order to allow them to be buffer-suballocations.
    _padding: vec4f,
};
@group(1) @binding(3)
var<uniform> draw_data: DrawDataUniformBuffer;

struct BatchUniformBuffer {
    world_from_obj: mat4x4f,
    flags: u32,
    depth_offset: f32,
    _padding: vec2u,
    outline_mask: vec2u,
    picking_layer_object_id: vec2u,
};
@group(2) @binding(0)
var<uniform> batch: BatchUniformBuffer;

// Flags
// See box_cloud.rs#BoxCloudBatchFlags
const FLAG_ENABLE_SHADING: u32 = 1u;

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

struct BoxData {
    center: vec3f,
    half_size: vec3f,
    color: vec4f,
    picking_instance_id: vec2u,
}

/// Read box data from textures
fn read_box_data(idx: u32) -> BoxData {
    let tex_size = textureDimensions(position_halfsize_texture);
    // Position/half-size texture contains 6 floats per box, packed as 2 texels
    // Texel 0: center.xyz, half_size.x
    // Texel 1: half_size.yz, padding, padding
    let texel_idx = idx * 2u;
    let pos_data_0 = textureLoad(position_halfsize_texture,
         vec2u(texel_idx % tex_size.x, texel_idx / tex_size.x), 0);
    let pos_data_1 = textureLoad(position_halfsize_texture,
         vec2u((texel_idx + 1u) % tex_size.x, (texel_idx + 1u) / tex_size.x), 0);

    let color_size = textureDimensions(color_texture);
    let color = textureLoad(color_texture,
         vec2u(idx % color_size.x, idx / color_size.x), 0);

    let picking_instance_id_size = textureDimensions(picking_instance_id_texture);
    let picking_instance_id = textureLoad(picking_instance_id_texture,
         vec2u(idx % picking_instance_id_size.x, idx / picking_instance_id_size.x), 0).xy;

    var data: BoxData;
    // Apply world_from_obj transformation to center
    let center_4d = batch.world_from_obj * vec4f(pos_data_0.xyz, 1.0);
    data.center = center_4d.xyz / center_4d.w;

    // Store half-size (we'll scale the unit box by this)
    data.half_size = vec3f(pos_data_0.w, pos_data_1.x, pos_data_1.y);

    data.color = color;
    data.picking_instance_id = picking_instance_id;
    return data;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOut {
    // Determine which box this vertex belongs to
    let box_idx = box_index(vertex_idx);

    // Read box data from textures
    let box_data = read_box_data(box_idx);

    // Get the vertex position within the unit box [-0.5, 0.5]³
    let unit_pos = box_vertex_position(vertex_idx);

    // Get the normal for this face
    let unit_normal = box_vertex_normal(vertex_idx);

    // Scale the unit box by the half-size and translate to center
    // First scale the unit position by half_size
    // The unit cube is [-0.5, 0.5], so multiply by 2.0 to get full size
    let local_pos = unit_pos * box_data.half_size * 2.0;

    // Then add to the center position (already in world space)
    let world_pos = box_data.center + local_pos;

    // Transform the normal to world space
    // For non-uniform scaling, we should use the inverse transpose of the upper 3x3
    // But for axis-aligned boxes with uniform or axis-aligned scaling, just scaling is fine
    let world_normal = normalize(unit_normal * box_data.half_size * 2.0);

    // Output
    var out: VertexOut;
    out.position = apply_depth_offset(frame.projection_from_world * vec4f(world_pos, 1.0), batch.depth_offset);
    out.world_position = world_pos;
    out.world_normal = world_normal;
    out.color = box_data.color;
    out.picking_instance_id = box_data.picking_instance_id;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    var shading = 1.0;

    if has_any_flag(batch.flags, FLAG_ENABLE_SHADING) {
        // Simple diffuse shading
        // Use normalized normal
        let normal = normalize(in.world_normal);

        // Simple headlight: light comes from camera
        let light_dir = normalize(frame.camera_position - in.world_position);

        // Lambertian diffuse with ambient
        let diffuse = max(dot(normal, light_dir), 0.0);
        let ambient = 0.4;
        shading = ambient + (1.0 - ambient) * diffuse;
    }

    // Apply shading to color
    let shaded_color = vec4f(in.color.rgb * shading, in.color.a);

    return shaded_color;
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) vec4u {
    return vec4u(batch.picking_layer_object_id, in.picking_instance_id);
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    return batch.outline_mask;
}
