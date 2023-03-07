#import <../types.wgsl>
#import <../global_bindings.wgsl>
#import <../screen_triangle_vertex.wgsl>

@group(1) @binding(0)
var voronoi_texture: texture_2d<f32>;

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    let resolution = textureDimensions(voronoi_texture).xy;
    let closest_positions = textureSample(voronoi_texture, nearest_sampler, in.texcoord);

    let to_closest_a = (closest_positions.xy - in.texcoord) * Vec2(resolution);
    let distance_a = length(abs(to_closest_a));

    let outline_a = saturate(8.0 - distance_a);

    // TODO: Second outline, coloring.
    return Vec4(1.0, 0.6, 0.0, 1.0) * outline_a ;

    // Useful for debugging
    //return Vec4(closest_positions.xy, 0.0, 1.0);
}
