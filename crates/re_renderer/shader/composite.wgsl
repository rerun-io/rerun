#import <./types.wgsl>
#import <./utils/srgb.wgsl>
#import <./global_bindings.wgsl>
#import <./screen_triangle_vertex.wgsl>

struct CompositeUniformBuffer {
    outline_color_layer_a: vec4f,
    outline_color_layer_b: vec4f,
    outline_radius_pixel: f32,
};
@group(1) @binding(0)
var<uniform> uniforms: CompositeUniformBuffer;

@group(1) @binding(1)
var color_texture: texture_2d<f32>;

@group(1) @binding(2)
var outline_voronoi_texture: texture_2d<f32>;

@fragment
fn main(in: FragmentInput) -> @location(0) vec4f {
    let resolution = vec2f(textureDimensions(color_texture).xy);
    let pixel_coordinates = floor(resolution * in.texcoord);

    // Note that we can't use a simple textureLoad using @builtin(position) here despite the lack of filtering.
    // The issue is that positions provided by @builtin(position) are not dependent on the set viewport,
    // but are about the location of the texel in the target texture.
    var color = textureSample(color_texture, nearest_sampler, in.texcoord).rgb;
    // TODO(andreas): Do something meaningful with values above 1
    color = clamp(color, ZERO.xyz, ONE.xyz);

    // Outlines
    {
        let closest_positions = textureSample(outline_voronoi_texture, nearest_sampler, in.texcoord);

        let distance_pixel_a = distance(pixel_coordinates, closest_positions.xy);
        let distance_pixel_b = distance(pixel_coordinates, closest_positions.zw);

        let sharpness = 1.0; // Fun to play around with, but not exposed yet.
        let outline_a = saturate((uniforms.outline_radius_pixel - distance_pixel_a) * sharpness);
        let outline_b = saturate((uniforms.outline_radius_pixel - distance_pixel_b) * sharpness);

        let outline_color_a = outline_a * uniforms.outline_color_layer_a;
        let outline_color_b = outline_b * uniforms.outline_color_layer_b;

        // Blend outlines with screen color.
        color = color * (1.0 - outline_color_a.a) + outline_color_a.rgb;
        color = color * (1.0 - outline_color_b.a) + outline_color_b.rgb;

        // Show only the outline. Useful for debugging.
        //color = outline_color_a.rgb;

        // Show the raw voronoi texture. Useful for debugging.
        //color = vec3f(closest_positions.xy / resolution, 0.0);
    }

    // Apply srgb gamma curve - this is necessary since the final eframe output does *not* have an srgb format.
    return vec4f(srgb_from_linear(color), 1.0);
}
