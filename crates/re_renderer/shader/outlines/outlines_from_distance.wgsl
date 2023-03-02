#import <../types.wgsl>
#import <../global_bindings.wgsl>
#import <../screen_triangle_vertex.wgsl>

@group(1) @binding(0)
var closest_pos_texture: texture_2d<f32>;

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    let resolution = textureDimensions(closest_pos_texture).xy;
    let closest_positions = textureSample(closest_pos_texture, nearest_sampler, in.texcoord);

    let to_closest_a = (closest_positions.xy - in.texcoord) * Vec2(resolution);
    let distance_a = length(abs(to_closest_a));

    let outline_a = saturate(4.0 - distance_a);

    return Vec4(1.0, 0.6, 0.0, 1.0) * outline_a ;
    //let l = distance_a / 32.0;
    //return Vec4(l, l, l, 1.0);

   // return Vec4(closest_positions.xy, 0.0, 1.0);
}
