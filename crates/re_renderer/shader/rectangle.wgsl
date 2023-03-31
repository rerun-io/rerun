#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./utils/depth_offset.wgsl>

struct UniformBuffer {
    /// Top left corner position in world space.
    top_left_corner_position: Vec3,

    /// Vector that spans up the rectangle from its top left corner along the u axis of the texture.
    extent_u: Vec3,

    /// Vector that spans up the rectangle from its top left corner along the v axis of the texture.
    extent_v: Vec3,

    depth_offset: f32,

    /// Tint multiplied with the texture color.
    multiplicative_tint: Vec4,

    outline_mask: UVec2,
};

@group(1) @binding(0)
var<uniform> rect_info: UniformBuffer;

@group(1) @binding(1)
var texture: texture_2d<f32>;

@group(1) @binding(2)
var texture_sampler: sampler;


struct VertexOut {
    @builtin(position) position: Vec4,
    @location(0) texcoord: Vec2,
};

@vertex
fn vs_main(@builtin(vertex_index) v_idx: u32) -> VertexOut {
    let texcoord = Vec2(f32(v_idx / 2u), f32(v_idx % 2u));
    let pos = texcoord.x * rect_info.extent_u + texcoord.y * rect_info.extent_v +
                rect_info.top_left_corner_position;

    var out: VertexOut;
    out.position = apply_depth_offset(frame.projection_from_world * Vec4(pos, 1.0), rect_info.depth_offset);
    out.texcoord = texcoord;

    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) Vec4 {
    let texture_color = textureSample(texture, texture_sampler, in.texcoord);
    return texture_color * rect_info.multiplicative_tint;
}

@fragment
fn fs_main_picking_layer(in: VertexOut) -> @location(0) UVec4 {
    return UVec4(0u, 0u, 0u, 0u); // TODO(andreas): Implement picking layer id pass-through.
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) UVec2 {
    return rect_info.outline_mask;
}
