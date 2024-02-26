#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./utils/srgb.wgsl>
#import <./utils/camera.wgsl>
#import <./screen_triangle_vertex.wgsl>

fn skybox_dark_srgb(dir: vec3f) -> vec3f {
    let rgb = dir * 0.5 + vec3f(0.5);
    return vec3f(0.05) + 0.20 * rgb;
}

fn skybox_light_srgb(dir: vec3f) -> vec3f {
    let rgb = dir * 0.5 + vec3f(0.5);
    return vec3f(0.85) + 0.15 * rgb;
}

// -----------------------------------------------
// Adapted from
// https://www.shadertoy.com/view/llVGzG
// Originally presented in:
// Jimenez 2014, "Next Generation Post-Processing in Call of Duty"

fn interleaved_gradient_noise(n: vec2f) -> f32 {
    let f = 0.06711056 * n.x + 0.00583715 * n.y;
    return fract(52.9829189 * fract(f));
}

fn dither_interleaved(rgb: vec3f, levels: f32, frag_coord: vec4<f32>) -> vec3f {
    var noise = interleaved_gradient_noise(frag_coord.xy);
    noise = noise - 0.5;
    return rgb + noise / (levels - 1.0);
}

// -----------------------------------------------

@fragment
fn main(in: FragmentInput, @builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4f {
    let camera_dir = camera_ray_direction_from_screenuv(in.texcoord);
    // Messing with direction a bit so it looks like in our old three-d based renderer (for easier comparison)
    let rgb = skybox_dark_srgb(camera_dir); // TODO(andreas): Allow switching to skybox_light

    // Apply dithering in gamma space.
    // TODO(andreas): Once we switch to HDR outputs, this can be removed.
    //                As of writing, the render target itself is (s)RGB8, so we need to dither while we still have maximum precision.
    let rgb_dithered = dither_interleaved(rgb, 256.0, frag_coord);

    return vec4f(linear_from_srgb(rgb_dithered), 1.0);
    //return vec4f(linear_from_srgb(rgb), 1.0); // Without dithering
    //return vec4f(camera_dir, 1.0);
}
