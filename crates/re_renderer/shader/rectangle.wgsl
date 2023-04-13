#import <./types.wgsl>
#import <./colormap.wgsl>
#import <./global_bindings.wgsl>
#import <./utils/depth_offset.wgsl>

// Keep in sync with mirror in rectangle.rs

// Which texture to read from?
const SAMPLE_TYPE_FLOAT = 1u;
const SAMPLE_TYPE_SINT  = 2u;
const SAMPLE_TYPE_UINT  = 3u;

// How do we do colormapping?
const COLOR_MAPPER_OFF      = 1u;
const COLOR_MAPPER_FUNCTION = 2u;
const COLOR_MAPPER_TEXTURE  = 3u;

struct UniformBuffer {
    /// Top left corner position in world space.
    top_left_corner_position: Vec3,

    /// Which colormap to use, if any
    colormap_function: u32,

    /// Vector that spans up the rectangle from its top left corner along the u axis of the texture.
    extent_u: Vec3,

    /// Which texture sample to use
    sample_type: u32,

    /// Vector that spans up the rectangle from its top left corner along the v axis of the texture.
    extent_v: Vec3,

    depth_offset: f32,

    /// Tint multiplied with the texture color.
    multiplicative_tint: Vec4,

    outline_mask: UVec2,

    /// Range of the texture values.
    /// Will be mapped to the [0, 1] range before we colormap.
    range_min_max: Vec2,

    color_mapper: u32,
};

@group(1) @binding(0)
var<uniform> rect_info: UniformBuffer;

@group(1) @binding(1)
var texture_sampler: sampler;

@group(1) @binding(2)
var texture_float: texture_2d<f32>;

@group(1) @binding(3)
var texture_sint: texture_2d<i32>;

@group(1) @binding(4)
var texture_uint: texture_2d<u32>;

@group(1) @binding(5)
var colormap_texture: texture_2d<f32>;


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
    // Sample the main texture:
    var sampled_value: Vec4;
    if rect_info.sample_type == SAMPLE_TYPE_FLOAT {
        sampled_value = textureSampleLevel(texture_float, texture_sampler, in.texcoord, 0.0); // TODO(emilk): support mipmaps
    } else if rect_info.sample_type == SAMPLE_TYPE_SINT {
        let icoords = IVec2(in.texcoord * Vec2(textureDimensions(texture_sint).xy));
        sampled_value = Vec4(textureLoad(texture_sint, icoords, 0));
    } else if rect_info.sample_type == SAMPLE_TYPE_UINT {
        let icoords = IVec2(in.texcoord * Vec2(textureDimensions(texture_uint).xy));
        sampled_value = Vec4(textureLoad(texture_uint, icoords, 0));
    } else {
        return ERROR_RGBA; // unknown sample type
    }

    // Normalize the sample:
    let range = rect_info.range_min_max;
    let normalized_value: Vec4 = (sampled_value - range.x) / (range.y - range.x);

    // TODO(emilk): apply a user-specified gamma-curve here

    // Apply colormap, if any:
    var texture_color: Vec4;
    if rect_info.color_mapper == COLOR_MAPPER_OFF {
        texture_color = normalized_value;
    } else if rect_info.color_mapper == COLOR_MAPPER_FUNCTION {
        let rgb = colormap_linear(rect_info.colormap_function, normalized_value.r);
        texture_color = Vec4(rgb, 1.0);
    } else if rect_info.color_mapper == COLOR_MAPPER_TEXTURE {
        let colormap_size = textureDimensions(colormap_texture).xy;
        let color_index = normalized_value.r * f32(colormap_size.x * colormap_size.y);
        // TODO(emilk): interpolate between neighboring colors for non-integral color indices
        let color_index_i32 = i32(color_index);
        let x = color_index_i32 % colormap_size.x;
        let y = color_index_i32 / colormap_size.x;
        texture_color = textureLoad(colormap_texture, IVec2(x, y), 0);
    } else {
        return ERROR_RGBA; // unknown color mapper
    }

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
