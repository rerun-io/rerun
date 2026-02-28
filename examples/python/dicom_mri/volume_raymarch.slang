// Volumetric raymarching fragment shader for 3D medical data.
//
// This Slang shader raymarches through a 3D texture bound to a mesh's
// bounding box, using front-to-back alpha compositing with a transfer function.
//
// Compile to WGSL with: slangc volume_raymarch.slang -target wgsl -o volume_raymarch.wgsl
// Extract reflection: slangc volume_raymarch.slang -target wgsl -dump-reflection-json

// Shader parameters (bound as custom bind group)
struct VolumeParams {
    float density_scale;
    float2 value_range;
};

uniform VolumeParams params;
Texture3D<float> volume_data;
SamplerState volume_sampler;

// Simple transfer function: intensity to color
float3 transferFunction(float intensity) {
    // Cool-to-warm colormap
    if (intensity < 0.25) {
        float t = intensity / 0.25;
        return lerp(float3(0.0, 0.0, 0.2), float3(0.0, 0.5, 0.8), t);
    } else if (intensity < 0.5) {
        float t = (intensity - 0.25) / 0.25;
        return lerp(float3(0.0, 0.5, 0.8), float3(0.9, 0.9, 0.9), t);
    } else if (intensity < 0.75) {
        float t = (intensity - 0.5) / 0.25;
        return lerp(float3(0.9, 0.9, 0.9), float3(0.9, 0.7, 0.2), t);
    } else {
        float t = (intensity - 0.75) / 0.25;
        return lerp(float3(0.9, 0.7, 0.2), float3(0.8, 0.1, 0.1), t);
    }
}

struct VertexOutput {
    float4 position : SV_Position;
    float2 texcoord : TEXCOORD0;
    float3 normal : NORMAL0;
    float3 world_position : TEXCOORD1;
};

// Entry point: the custom mesh shader system expects fs_main.
[shader("fragment")]
float4 fs_main(VertexOutput input) : SV_Target {
    float3 ray_origin = float3(input.texcoord, 0.0);
    float3 ray_dir = float3(0.0, 0.0, 1.0);

    int num_steps = 128;
    float step_size = 1.0 / float(num_steps);

    float3 accumulated_color = float3(0.0);
    float accumulated_alpha = 0.0;

    float range_min = params.value_range.x;
    float range_max = params.value_range.y;
    float range_extent = max(range_max - range_min, 0.001);

    for (int i = 0; i < num_steps; i++) {
        if (accumulated_alpha >= 0.95) break;

        float t = float(i) * step_size;
        float3 sample_pos = ray_origin + ray_dir * t;

        float raw_value = volume_data.Sample(volume_sampler, sample_pos).r;
        float normalized = saturate((raw_value - range_min) / range_extent);

        float sample_alpha = normalized * params.density_scale * step_size;
        float3 sample_color = transferFunction(normalized);

        // Front-to-back compositing
        accumulated_color += (1.0 - accumulated_alpha) * sample_alpha * sample_color;
        accumulated_alpha += (1.0 - accumulated_alpha) * sample_alpha;
    }

    return float4(accumulated_color, accumulated_alpha);
}
