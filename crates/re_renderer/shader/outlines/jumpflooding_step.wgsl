#import <../types.wgsl>
#import <../global_bindings.wgsl>
#import <../screen_triangle_vertex.wgsl>

@group(0) @binding(0)
var distance_texture: texture_2d<f32>;

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    return Vec4(1.0, 1.0, 1.0, 1.0);
}
