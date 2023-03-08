#import <../types.wgsl>
#import <../global_bindings.wgsl>
#import <../screen_triangle_vertex.wgsl>
#import <../utils/srgb.wgsl>

@group(1) @binding(0)
var voronoi_texture: texture_2d<f32>;

struct OutlineConfigUniformBuffer {
    color_layer_a: Vec4,
    color_layer_b: Vec4,
    outline_radius_pixel: f32,
};
@group(1) @binding(1)
var<uniform> uniforms: OutlineConfigUniformBuffer;

@fragment
fn main(in: FragmentInput) -> @location(0) Vec4 {
    let resolution = Vec2(textureDimensions(voronoi_texture).xy);
    let pixel_coordinates = resolution * in.texcoord;
    let closest_positions = textureSample(voronoi_texture, nearest_sampler, in.texcoord);
    let to_closest_a_and_b = (closest_positions - pixel_coordinates.xyxy);
    let distance_pixel_a = length(to_closest_a_and_b.xy);
    let distance_pixel_b = length(to_closest_a_and_b.zw);

    let sharpness = 1.0; // Fun to play around with, but not exposed yet.
    let outline_a = saturate((uniforms.outline_radius_pixel - distance_pixel_a) * sharpness);
    let outline_b = saturate((uniforms.outline_radius_pixel - distance_pixel_b) * sharpness);

    let color_a = outline_a * uniforms.color_layer_a;
    let color_b = outline_b * uniforms.color_layer_b;

    // Blend B over A.
    let color = color_a * (1.0 - color_b.a) + color_b;
    return srgba_from_linear(color);

    // Show only the outline. Useful for debugging.
    //return Vec4(color.rgb, 1.0);

    // Show the raw voronoi texture. Useful for debugging.
    //return Vec4(closest_positions.xy / resolution, 0.0, 1.0);
}
