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

    let outline_a = saturate(8.0 - distance_a);

    return Vec4(outline_a, 0.0, 0.0, outline_a);

    //return Vec4(closest_positions.xy, 0.0, 1.0);
}
