#import <./types.wgsl>
#import <./utils/srgb.wgsl>

struct VertexOutput {
    @builtin(position) position: Vec4,
    @location(0) texcoord: Vec2,
};

// TODO(andreas): Move global bindings to shared include
@group(0) @binding(1)
var nearest_sampler: sampler;

@group(1) @binding(0)
var hdr_texture: texture_2d<f32>;

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    // Note that we can't use a simple textureLoad using @builtin(position) here despite the lack of filtering.
    // The issue is that positions provided by @builtin(position) are not dependent on the set viewport,
    // but are about the location of the texel in the target texture.
    let hdr = textureSample(hdr_texture, nearest_sampler, in.texcoord).rgb;
    // TODO(andreas): Do something meaningful with values above 1
    let hdr = clamp(hdr, vec3<f32>(0.0), vec3<f32>(1.0));
    return Vec4(srgb_from_linear(hdr), 1.0);
}
