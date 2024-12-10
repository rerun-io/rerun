#import <./types.wgsl>
#import <./utils/srgb.wgsl>
#import <./global_bindings.wgsl>
#import <./screen_triangle_vertex.wgsl>

struct CompositeUniformBuffer {
    outline_color_layer_a: vec4f,
    outline_color_layer_b: vec4f,
    outline_radius_pixel: f32,
    blend_with_background: u32,
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
    var color = textureSample(color_texture, nearest_sampler_clamped, in.texcoord);


    // TODO(andreas): We assume that the color from the texture does *not* have pre-multiplied alpha.
    // This is a brittle workaround for the alpha-to-coverage issue described in `ViewBuilder::MAIN_TARGET_ALPHA_TO_COVERAGE_COLOR_STATE`:
    // We need this because otherwise the feathered edges of alpha-to-coverage would be overly bright, as after
    // MSAA-resolve they end up with an unusually low alpha value relative to the color value.
    if uniforms.blend_with_background == 0 {
        // To not apply this hack needlessly and account for alpha from alpha to coverage, we have to ignore alpha values if blending is disabled.
        color = vec4f(color.rgb, 1.0);
    } else {
        color = vec4f(color.rgb * color.a, color.a);
    }

    // Outlines
    {
        let closest_positions = textureSample(outline_voronoi_texture, nearest_sampler_clamped, in.texcoord);

        let distance_pixel_a = distance(pixel_coordinates, closest_positions.xy);
        let distance_pixel_b = distance(pixel_coordinates, closest_positions.zw);

        let sharpness = 1.0; // Fun to play around with, but not exposed yet.
        let outline_a = saturate((uniforms.outline_radius_pixel - distance_pixel_a) * sharpness);
        let outline_b = saturate((uniforms.outline_radius_pixel - distance_pixel_b) * sharpness);

        let outline_color_a = outline_a * uniforms.outline_color_layer_a;
        let outline_color_b = outline_b * uniforms.outline_color_layer_b;

        // Blend outlines with screen color:
        if false {
            // Normal blending with premul alpha.
            // Problem: things that are both hovered and selected will get double outlines,
            // which can look really ugly if e.g. the selection is dark blue and the hover is bright white.
            color = color * (1.0 - outline_color_a.a) + outline_color_a;
            color = color * (1.0 - outline_color_b.a) + outline_color_b;
        } else {
            // Add the two outline colors, then blend that in:
            let outline_color_sum = saturate(outline_color_a + outline_color_b);
            color = color * (1.0 - outline_color_sum.a) + outline_color_sum;
        }

        // Show only the outline. Useful for debugging.
        //color = outline_color_b;

        // Show the raw voronoi texture. Useful for debugging.
        //color = vec4f(closest_positions.xy / resolution, 0.0, 1.0);
    }

    color = saturate(color); // TODO(andreas): Do something meaningful with values above 1

    // Apply srgb gamma curve - this is necessary since the final eframe output does *not* have an srgb format.
    // We can't do this with pre-multiplied alpha, because it would shift how additive the color is.
    //
    // Note that egui doing blending in non-linear is a workaround for otherwise poor text rendering, see:
    // * https://github.com/emilk/egui/pull/2071
    // * http://hikogui.org/2022/10/24/the-trouble-with-anti-aliasing.html
    color = premultiplied_to_unmultiplied(color);
    color = srgba_from_linear(color);
    color = vec4f(color.rgb * color.a, color.a);

    return color;
}

fn premultiplied_to_unmultiplied(color: vec4f) -> vec4f {
    if (color.a == 0.0) {
        return vec4f(0.0);
    }
    return vec4f(color.rgb / color.a, color.a);
}
