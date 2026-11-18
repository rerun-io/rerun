#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

struct UniformBuffer {
    bayer_pattern: u32,
};

@group(0) @binding(0)
var<uniform> uniform_buffer: UniformBuffer;

@group(0) @binding(1)
var input_texture: texture_2d<u32>;

// see `enum BayerPattern` in `crates/store/re_sdk_types/src/bayer_pattern.rs`.
const BAYER_PATTERN_RGGB = 0u;
const BAYER_PATTERN_BGGR = 1u;
const BAYER_PATTERN_GBRG = 2u;
const BAYER_PATTERN_GRBG = 3u;

/// The color channel sampled at the given pixel, where 0 is red, 1 is green and 2 is blue.
fn channel_at(bayer_pattern: u32, coords: vec2u) -> u32 {
    var channels: vec4u;
    switch (bayer_pattern) {
        case BAYER_PATTERN_RGGB: { channels = vec4u(0u, 1u, 1u, 2u); }
        case BAYER_PATTERN_BGGR: { channels = vec4u(2u, 1u, 1u, 0u); }
        case BAYER_PATTERN_GBRG: { channels = vec4u(1u, 2u, 0u, 1u); }
        case BAYER_PATTERN_GRBG: { channels = vec4u(1u, 0u, 2u, 1u); }
        default: { channels = vec4u(0u); }
    }
    return channels[(coords.y % 2u) * 2u + coords.x % 2u];
}

/// Bilinear demosaicing of a single pixel, in the value range of the input data.
///
/// GPU equivalent of `BayerPattern::demosaic_at` in `re_sdk_types`.
fn demosaic(coords: vec2u) -> vec3f {
    let bayer_pattern = uniform_buffer.bayer_pattern;
    let own_channel = channel_at(bayer_pattern, coords);

    let min_coords = max(vec2i(coords) - vec2i(1), vec2i(0));
    let max_coords = min(vec2i(coords) + vec2i(1), vec2i(textureDimensions(input_texture)) - vec2i(1));

    var sums = vec3f(0.0);
    var counts = vec3f(0.0);
    for (var y = min_coords.y; y <= max_coords.y; y += 1) {
        for (var x = min_coords.x; x <= max_coords.x; x += 1) {
            let neighbor = vec2u(u32(x), u32(y));
            let channel = channel_at(bayer_pattern, neighbor);
            if channel != own_channel {
                sums[channel] += f32(textureLoad(input_texture, neighbor, 0).r);
                counts[channel] += 1.0;
            }
        }
    }

    var rgb = sums / max(counts, vec3f(1.0));
    rgb[own_channel] = f32(textureLoad(input_texture, coords, 0).r);
    return rgb;
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    let coords = vec2u(vec2f(textureDimensions(input_texture)) * in.texcoord);
    let rgb = demosaic(coords);
    return vec4f(rgb / 255.0, 1.0);
}
