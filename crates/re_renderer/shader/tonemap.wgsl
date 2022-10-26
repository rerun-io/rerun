struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texcoord: vec2<f32>,
};

// TODO(andreas): Move global bindings to shared include
@group(0) @binding(1)
var nearest_sampler: sampler;

@group(1) @binding(0)
var hdr_texture: texture_2d<f32>;

/// 0-1 gamma from 0-1 linear
fn srgb_from_linear(color_linear: vec3<f32>) -> vec3<f32> {
    var selector = ceil(color_linear - 0.0031308);
    var under = 12.92 * color_linear;
    var over = 1.055 * pow(color_linear, vec3<f32>(0.41666)) - 0.055;
    return mix(under, over, selector);
}

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Note that we can't use a simple textureLoad using @builtin(position) here despite the lack of filtering.
    // The issue is that positions provided by @builtin(position) are not dependent on the set viewport,
    // but are about the location of the texel in the target texture.
    var hdr = textureSample(hdr_texture, nearest_sampler, in.texcoord);
    return vec4<f32>(srgb_from_linear(hdr.rgb), 1.0);
}
