#import <../types.wgsl>
#import <../global_bindings.wgsl>
#import <../screen_triangle_vertex.wgsl>

@group(0) @binding(0)
var mask_texture: texture_multisampled_2d<u32>;

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    let resolution = textureDimensions(mask_texture);

    // Assume default sampling pattern for VK_SAMPLE_COUNT_4_BIT
    // https://registry.khronos.org/vulkan/specs/1.3-khr-extensions/html/chap25.html#primsrast-multisampling
    //let num_samples = textureNumSamples(mask_texture);
    // Should we assert somehow on textureNumSamples here

    let mask = textureLoad(mask_texture, UVec2(Vec2(resolution) * in.texcoord), 0);
    return Vec4(f32(mask.r));
}
