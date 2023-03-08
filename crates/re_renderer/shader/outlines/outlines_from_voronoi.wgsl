#import <../types.wgsl>
#import <../global_bindings.wgsl>
#import <../screen_triangle_vertex.wgsl>

@group(1) @binding(0)
var voronoi_texture: texture_2d<f32>;

struct OutlineConfigUniformBuffer {
    color_layer_a: Vec4,
    color_layer_b: Vec4,
    outline_thickness_pixel: f32,
};
@group(1) @binding(1)
var<uniform> uniforms: OutlineConfigUniformBuffer;

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    let resolution = Vec2(textureDimensions(voronoi_texture).xy);
    let closest_positions = textureSample(voronoi_texture, nearest_sampler, in.texcoord);

    let to_closest_a_and_b = (closest_positions - Vec4(in.texcoord, in.texcoord));
    let to_closest_a_and_b_pixel = to_closest_a_and_b * resolution.xyxy;
    let distance_pixel_a = length(to_closest_a_and_b_pixel.xy);
    let distance_pixel_b = length(to_closest_a_and_b_pixel.zw);

    let sharpness = 1.0; // Fun to play around with, but not exposed yet.
    let outline_a = saturate((uniforms.outline_thickness_pixel - distance_pixel_a) * sharpness);
    let outline_b = saturate((uniforms.outline_thickness_pixel - distance_pixel_b) * sharpness);

    let color_a = outline_a * uniforms.color_layer_a;
    let color_b = outline_b * uniforms.color_layer_b;

    // Blend B over A.
    return color_a * (1.0 - color_b.a) + color_b;

    // Show the raw voronoi texture. Useful for debugging.
    //return Vec4(closest_positions.xy, 0.0, 1.0);
}
