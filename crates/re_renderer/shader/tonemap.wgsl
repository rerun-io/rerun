#import <./utils/srgb.wgsl>

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texcoord: vec2<f32>,
};

// TODO(andreas): Move global bindings to shared include
@group(0) @binding(1)
var nearest_sampler: sampler;

@group(1) @binding(0)
var hdr_texture: texture_2d<f32>;

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Note that we can't use a simple textureLoad using @builtin(position) here despite the lack of filtering.
    // The issue is that positions provided by @builtin(position) are not dependent on the set viewport,
    // but are about the location of the texel in the target texture.
    var hdr = textureSample(hdr_texture, nearest_sampler, in.texcoord);
    return vec4<f32>(srgb_from_linear(hdr.rgb), 1.0);
}
