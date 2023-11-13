#import <./types.wgsl>

// Keep in sync with mirror in rectangle.rs

// Which texture to read from?
const SAMPLE_TYPE_FLOAT = 1u;
const SAMPLE_TYPE_SINT  = 2u;
const SAMPLE_TYPE_UINT  = 3u;
const SAMPLE_TYPE_NV12  = 4u;

// How do we do colormapping?
const COLOR_MAPPER_OFF_GRAYSCALE = 1u;
const COLOR_MAPPER_OFF_RGB       = 2u;
const COLOR_MAPPER_FUNCTION      = 3u;
const COLOR_MAPPER_TEXTURE       = 4u;

const FILTER_NEAREST  = 1u;
const FILTER_BILINEAR = 2u;

struct UniformBuffer {
    /// Top left corner position in world space.
    top_left_corner_position: vec3f,

    /// Which colormap to use, if any
    colormap_function: u32,

    /// Vector that spans up the rectangle from its top left corner along the u axis of the texture.
    extent_u: vec3f,

    /// Which texture sample to use
    sample_type: u32,

    /// Vector that spans up the rectangle from its top left corner along the v axis of the texture.
    extent_v: vec3f,

    depth_offset: f32,

    /// Tint multiplied with the texture color.
    multiplicative_tint: vec4f,

    outline_mask: vec2u,

    /// Range of the texture values.
    /// Will be mapped to the [0, 1] range before we colormap.
    range_min_max: vec2f,

    color_mapper: u32,

    /// Exponent to raise the normalized texture value.
    /// Inverse brightness.
    gamma: f32,

    minification_filter: u32,
    magnification_filter: u32,

    /// Boolean: decode 0-1 sRGB gamma to linear space before filtering?
    decode_srgb: u32,

    /// Boolean: multiply RGB with alpha before filtering
    multiply_rgb_with_alpha: u32,
};

@group(1) @binding(0)
var<uniform> rect_info: UniformBuffer;

@group(1) @binding(1)
var texture_float: texture_2d<f32>;

@group(1) @binding(2)
var texture_sint: texture_2d<i32>;

@group(1) @binding(3)
var texture_uint: texture_2d<u32>;

@group(1) @binding(4)
var colormap_texture: texture_2d<f32>;

@group(1) @binding(5)
var texture_float_filterable: texture_2d<f32>;

struct VertexOut {
    @builtin(position) position: vec4f,
    @location(0) texcoord: vec2f,
};

// The fragment and vertex shaders are in two separate files in order
// to work around this bug: https://github.com/gfx-rs/naga/issues/1743
