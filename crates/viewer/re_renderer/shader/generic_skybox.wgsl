#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./utils/srgb.wgsl>
#import <./utils/camera.wgsl>
#import <./screen_triangle_vertex.wgsl>

struct UniformBuffer {
    // See `GenericSkyboxType` in `generic_skybox.rs`
    background_type: u32,
    _padding: vec3f,
}

const GRADIENT_DARK: u32 = 0u;
const GRADIENT_BRGHT: u32 = 1u;

@group(1) @binding(0)
var<uniform> uniforms: UniformBuffer;

fn skybox_dark_srgb(dir: vec3f) -> vec3f {
    let rgb = dir * 0.5 + vec3f(0.5);
    return vec3f(0.05) + 0.20 * rgb;
}

fn skybox_light_srgb(dir: vec3f) -> vec3f {
    let rgb = dir * 0.5 + vec3f(0.5);
    return vec3f(0.7) + 0.20 * rgb;
}

// -----------------------------------------------
// Adapted from
// https://www.shadertoy.com/view/llVGzG
// Originally presented in:
// Jimenez 2014, "Next Generation Post-Processing in Call of Duty"
//
// A good overview can be found in
// https://blog.demofox.org/2022/01/01/interleaved-gradient-noise-a-different-kind-of-low-discrepancy-sequence/

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

    var rgb: vec3f;
    if uniforms.background_type == GRADIENT_DARK {
        rgb = skybox_dark_srgb(camera_dir);
    } else {
        rgb = skybox_light_srgb(camera_dir);
    }

    if frame.deterministic_rendering == 1 {
        return vec4f(linear_from_srgb(rgb), 1.0); // Without dithering
    } else {
        // Apply dithering in gamma space.
        // TODO(andreas): Once we switch to HDR outputs, this can be removed.
        //                As of writing, the render target itself is (s)RGB8, so we need to dither while we still have maximum precision.
        var rgb_gamma_dithered = dither_interleaved(rgb, 256.0, frag_coord);

        return vec4f(linear_from_srgb(rgb_gamma_dithered), 1.0);
    }
}
