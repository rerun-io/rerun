struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texcoord: vec2<f32>,
};

@group(0) @binding(0)
var hdr_texture: texture_2d<f32>;

@group(0) @binding(1)
var nearest_sampler: sampler;

/// 0-1 gamma from 0-1 linear
fn linear_to_srgb(color_linear: vec3<f32>) -> vec3<f32>  {
    var selector = ceil(color_linear - 0.0031308);
    var under = 12.92 * color_linear;
    var over = 1.055 * pow(color_linear, vec3<f32>(0.41666)) - 0.055;
    return mix(under, over, selector);
}

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Note that textureLoad with pixel coordinates won't work for us since seems to ignore viewport cutouts which we need to support here
    var hdr = textureSample(hdr_texture, nearest_sampler, in.texcoord);
    return vec4<f32>(linear_to_srgb(hdr.rgb), 1.0);
}
