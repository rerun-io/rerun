#import <../types.wgsl>

struct VertexOutput {
    @builtin(position) position: Vec4,
    @location(0) texcoord: Vec2,
};

@group(1) @binding(0)
var mask_texture: texture_multisampled_2d<u32>;

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    let resolution =  textureDimensions(mask_texture);
    let mask = textureLoad(mask_texture, UVec2(Vec2(resolution) * in.texcoord), 0);
    return Vec4(Vec3(mask.rgb), 1.0) * 0.5;
}
