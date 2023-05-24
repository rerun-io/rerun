#import <./types.wgsl>

// Keep in sync with mirror in rectangle.rs

// Which texture to read from?
const SAMPLE_TYPE_FLOAT = 1u;
const SAMPLE_TYPE_SINT  = 2u;
const SAMPLE_TYPE_UINT  = 3u;

// How do we do colormapping?
const COLOR_MAPPER_OFF      = 1u;
const COLOR_MAPPER_FUNCTION = 2u;
const COLOR_MAPPER_TEXTURE  = 3u;

const FILTER_NEAREST = 1u;
const FILTER_BILINEAR = 2u;

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

    /// Exponent to raise the normalized texture value.
    /// Inverse brightness.
    gamma: f32,

    minification_filter: u32,
    magnification_filter: u32,

    /// Boolean: decode 0-1 sRGB gamma to linear space before filtering?
    decode_srgb: u32,
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

@group(1) @binding(6)
var texture_float_filterable: texture_2d<f32>;

struct VertexOut {
    @builtin(position) position: Vec4,
    @location(0) texcoord: Vec2,
};

// The fragment and vertex shaders are in two separate files in order
// to work around this bug: https://github.com/gfx-rs/naga/issues/1743
