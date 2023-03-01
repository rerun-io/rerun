#import <../types.wgsl>
#import <../global_bindings.wgsl>
#import <../screen_triangle_vertex.wgsl>

@group(1) @binding(0)
var distance_texture: texture_2d<f32>;

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    let resolution = textureDimensions(distance_texture);
    let distances = textureSample(distance_texture, nearest_sampler, in.texcoord).rgba;
    return Vec4(distances.rg, 1.0, 1.0);
}
