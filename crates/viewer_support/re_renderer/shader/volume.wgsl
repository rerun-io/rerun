#import <./colormap.wgsl>
#import <./global_bindings.wgsl>
#import <./utils/camera.wgsl>

// Direct volume rendering inside a unit cube whose coordinates are also texture coordinates.
// Each fragment marches through the cube and composites density samples from front to back.

struct UniformBuffer {
    world_from_volume: mat4x4f,
    volume_from_world: mat4x4f,
    value_range: vec2f,
    optical_density: f32,
    colormap: u32,
    outline_mask: vec2u,
    gamma: f32,
    _reserved: u32,
    picking_layer_id: vec4u,
};

@group(1) @binding(0)
var scene_depth_texture: texture_depth_2d;

@group(2) @binding(0)
var<uniform> volume: UniformBuffer;

@group(2) @binding(1)
var density_texture: texture_3d<f32>;

struct VertexOut {
    @builtin(position)
    position: vec4f,

    @location(0)
    volume_position: vec3f,
};

@vertex
fn vs_main(@location(0) volume_position: vec3f) -> VertexOut {
    var out: VertexOut;
    out.volume_position = volume_position;
    let world_position = volume.world_from_volume * vec4f(out.volume_position, 1.0);
    out.position = frame.projection_from_world * world_position;
    return out;
}

// Returns the ray's entry and exit distances through the unit cube.
fn ray_box_intersection(origin: vec3f, direction: vec3f) -> vec2f {
    let safe_direction = select(vec3f(1e-20), direction, abs(direction) > vec3f(1e-20));
    let t0 = -origin / safe_direction;
    let t1 = (vec3f(1.0) - origin) / safe_direction;
    let t_min = min(t0, t1);
    let t_max = max(t0, t1);
    return vec2f(max(max(t_min.x, t_min.y), t_min.z), min(min(t_max.x, t_max.y), t_max.z));
}

const MAX_STEPS: u32 = 512u;
// Stop marching once less than ten percent of the background remains visible.
const MIN_TRANSMITTANCE = 0.1;
// Minimum accumulated opacity for outlines and picking.
const PICKING_AND_OUTLINE_ALPHA_CUTOFF = 0.08;

fn distance_to_scene_depth(world_ray: Ray, ray: Ray, pixel: vec2i) -> f32 {
    let scene_depth = textureLoad(scene_depth_texture, pixel, 0);
    if scene_depth <= 0.0 {
        return f32max;
    }

    let world_hit = camera_ray_depth_hit(world_ray, scene_depth);
    let volume_hit = (volume.volume_from_world * vec4f(world_hit, 1.0)).xyz;
    return dot(volume_hit - ray.origin, ray.direction);
}

fn sample_density(position: vec3f) -> f32 {
    let density = textureSampleLevel(density_texture, trilinear_sampler_repeat, position, 0.0).r;
    if density < volume.value_range.x || density > volume.value_range.y {
        return 0.0;
    }

    let normalized_density = (density - volume.value_range.x) / (volume.value_range.y - volume.value_range.x);
    return pow(normalized_density, volume.gamma);
}

// Vary the ray phase per pixel to replace coherent sampling bands with fine noise.
fn dither(pixel: vec2i) -> f32 {
    return fract(sin(dot(vec2f(pixel), vec2f(12.9898, 78.233))) * 43758.5453);
}

fn volume_ray(world_ray: Ray) -> Ray {
    let ray_origin = (volume.volume_from_world * vec4f(world_ray.origin, 1.0)).xyz;
    // Normalize after transforming so marching distances remain in normalized volume units.
    let ray_direction = normalize((volume.volume_from_world * vec4f(world_ray.direction, 0.0)).xyz);
    return Ray(ray_origin, ray_direction);
}

struct RaymarchResult {
    color: vec4f,
    hit_position: vec3f,
};

fn raymarch(ray: Ray, pixel: vec2i, max_distance: f32) -> RaymarchResult {
    let ray_origin = ray.origin;
    let ray_direction = ray.direction;
    let intersection = ray_box_intersection(ray_origin, ray_direction);
    // Opaque scene geometry limits how far the volume may contribute along this ray.
    let ray_start = max(intersection.x, 0.0);
    let ray_end = min(intersection.y, max_distance);
    let ray_length = ray_end - ray_start;
    if ray_length <= 0.0 {
        return RaymarchResult(vec4f(0.0), vec3f(0.0));
    }

    let dimensions = vec3f(textureDimensions(density_texture));
    // Sample approximately once per crossed voxel, subject to the global step limit.
    let voxel_ray_length = ray_length * length(ray_direction * dimensions);
    let num_steps = min(max(u32(ceil(voxel_ray_length)), 1u), MAX_STEPS);
    let step_size = ray_length / f32(num_steps);
    let half_texel = 0.5 / dimensions;

    var hit_position = vec3f(0.0);
    var accumulated = vec3f(0.0);
    var transmittance = 1.0;
    // Start near each cell center, with a stable per-pixel offset.
    let ray_offset = 0.5 + dither(pixel) * 0.5;
    for (var step = 0u; step < num_steps; step += 1u) {
        if transmittance < MIN_TRANSMITTANCE {
            break;
        }

        let distance = ray_start + (f32(step) + ray_offset) * step_size;
        let sample_position = clamp(
            ray_origin + ray_direction * distance,
            half_texel,
            vec3f(1.0) - half_texel,
        );
        let density = sample_density(sample_position);
        if density > 0.0 {
            let color = colormap_linear(volume.colormap, density);
            // Beer-Lambert extinction keeps opacity independent of the sampling rate.
            let sample_alpha = 1.0 - exp(-volume.optical_density * color.a * density * step_size);

            // Front-to-back premultiplied-alpha compositing.
            accumulated += transmittance * sample_alpha * color.rgb;
            if 1.0 - transmittance < PICKING_AND_OUTLINE_ALPHA_CUTOFF {
                hit_position = ray_origin + ray_direction * distance;
            }
            transmittance *= 1.0 - sample_alpha;
        }
    }

    return RaymarchResult(vec4f(accumulated, 1.0 - transmittance), hit_position);
}

fn raymarch_scene(surface_position: vec3f, pixel: vec2i) -> vec4f {
    let world_position = (volume.world_from_volume * vec4f(surface_position, 1.0)).xyz;
    let world_ray = camera_ray_to_world_pos(world_position);
    let ray = volume_ray(world_ray);
    return raymarch(ray, pixel, distance_to_scene_depth(world_ray, ray, pixel)).color;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    let color = raymarch_scene(in.volume_position, vec2i(in.position.xy));
    return color;
}

@fragment
fn fs_main_outline_mask(in: VertexOut) -> @location(0) vec2u {
    if raymarch_scene(in.volume_position, vec2i(in.position.xy)).a < PICKING_AND_OUTLINE_ALPHA_CUTOFF {
        discard;
    }
    return volume.outline_mask;
}

struct PickingOutput {
    @location(0) id: vec4u,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_main_picking_layer(in: VertexOut) -> PickingOutput {
    // Picking uses a cropped camera and its own depth buffer, without sampling scene depth.
    let world_position = (volume.world_from_volume * vec4f(in.volume_position, 1.0)).xyz;
    let world_ray = camera_ray_to_world_pos(world_position);
    let result = raymarch(volume_ray(world_ray), vec2i(in.position.xy), f32max);
    if result.color.a < PICKING_AND_OUTLINE_ALPHA_CUTOFF {
        discard;
    }
    // Depth identifies the first visible sample rather than the cube's back face.
    let clip_position = frame.projection_from_world * volume.world_from_volume * vec4f(result.hit_position, 1.0);
    return PickingOutput(volume.picking_layer_id, clip_position.z / clip_position.w);
}
