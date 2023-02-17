#import <./types.wgsl>
#import <./utils/srgb.wgsl>
#import <./global_bindings.wgsl>

struct VertexOutput {
    @builtin(position) position: Vec4,
    @location(0) texcoord: Vec2,
};

@group(1) @binding(0)
var input_texture: texture_2d<f32>;

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    // Note that we can't use a simple textureLoad using @builtin(position) here despite the lack of filtering.
    // The issue is that positions provided by @builtin(position) are not dependent on the set viewport,
    // but are about the location of the texel in the target texture.
    var input = textureSample(input_texture, nearest_sampler, in.texcoord).rgb;
    // TODO(andreas): Do something meaningful with values above 1
    input = clamp(input, ZERO.xyz, ONE.xyz);

    // Convert to srgb - this is necessary since the final eframe output does *not* have an srgb format.
    // Note that the input here is assumed to be linear - if the input texture was an srgb texture it would have been converted on load.
    return Vec4(srgb_from_linear(input), 1.0);
}
