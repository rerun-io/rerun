// Volumetric raymarching fragment shader for 3D medical data.
//
// This shader raymarches through a 3D texture bound to a mesh's bounding box,
// using front-to-back alpha compositing with a simple transfer function.
//
// Pre-compiled from volume_raymarch.slang for use without Slang installed.

// ---- Custom bind group (group 2) ----

// Uniform buffer with shader parameters.
struct VolumeParams {
    density_scale: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
    value_range: vec2<f32>,
};

@group(2) @binding(0)
var<uniform> params: VolumeParams;

@group(2) @binding(1)
var volume_texture: texture_3d<f32>;

@group(2) @binding(101)
var volume_sampler: sampler;

// ---- Entry point ----

// The fragment entry point for custom mesh shaders must be named `fs_main`.
// It receives the interpolated vertex data from the standard mesh vertex shader.

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texcoord: vec2<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) world_position: vec3<f32>,
};

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Use the mesh UV coordinates to determine the ray entry point.
    // For a unit cube mesh, texcoords map directly to volume coordinates.
    let ray_origin = vec3<f32>(in.texcoord, 0.0);
    let ray_dir = vec3<f32>(0.0, 0.0, 1.0);

    let num_steps: i32 = 128;
    let step_size: f32 = 1.0 / f32(num_steps);

    var accumulated_color = vec3<f32>(0.0);
    var accumulated_alpha: f32 = 0.0;

    let range_min = params.value_range.x;
    let range_max = params.value_range.y;
    let range_extent = max(range_max - range_min, 0.001);

    for (var i: i32 = 0; i < num_steps; i = i + 1) {
        if accumulated_alpha >= 0.95 {
            break;
        }

        let t = f32(i) * step_size;
        let sample_pos = ray_origin + ray_dir * t;

        // Sample the 3D volume texture.
        let raw_value = textureSample(volume_texture, volume_sampler, sample_pos).r;

        // Normalize to [0, 1] using the value range.
        let normalized = clamp((raw_value - range_min) / range_extent, 0.0, 1.0);

        // Simple transfer function: map intensity to color and opacity.
        let sample_alpha = normalized * params.density_scale * step_size;
        let sample_color = transfer_function(normalized);

        // Front-to-back compositing.
        accumulated_color = accumulated_color + (1.0 - accumulated_alpha) * sample_alpha * sample_color;
        accumulated_alpha = accumulated_alpha + (1.0 - accumulated_alpha) * sample_alpha;
    }

    return vec4<f32>(accumulated_color, accumulated_alpha);
}

// Simple grayscale-to-color transfer function for medical data.
fn transfer_function(intensity: f32) -> vec3<f32> {
    // Cool-to-warm colormap: dark blue → cyan → white → yellow → red
    if intensity < 0.25 {
        let t = intensity / 0.25;
        return mix(vec3<f32>(0.0, 0.0, 0.2), vec3<f32>(0.0, 0.5, 0.8), t);
    } else if intensity < 0.5 {
        let t = (intensity - 0.25) / 0.25;
        return mix(vec3<f32>(0.0, 0.5, 0.8), vec3<f32>(0.9, 0.9, 0.9), t);
    } else if intensity < 0.75 {
        let t = (intensity - 0.5) / 0.25;
        return mix(vec3<f32>(0.9, 0.9, 0.9), vec3<f32>(0.9, 0.7, 0.2), t);
    } else {
        let t = (intensity - 0.75) / 0.25;
        return mix(vec3<f32>(0.9, 0.7, 0.2), vec3<f32>(0.8, 0.1, 0.1), t);
    }
}
