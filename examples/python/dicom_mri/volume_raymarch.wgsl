// Volumetric raymarching fragment shader for 3D medical data.
//
// This shader raymarches through a 3D texture bound to a mesh's bounding box,
// using view-dependent rays with front-to-back alpha compositing.
//
// The mesh's object-space positions define the [0,1]^3 volume coordinate system.
// The camera position and direction from global uniforms (group 0) are used to
// compute per-pixel ray directions for proper 3D perspective.

// ---- Global uniforms (group 0) ----

struct FrameUniformBuffer {
    view_from_world: mat4x3<f32>,
    projection_from_view: mat4x4<f32>,
    projection_from_world: mat4x4<f32>,
    camera_position: vec3<f32>,
    pixel_world_size_from_camera_distance: f32,
    camera_forward: vec3<f32>,
    pixels_from_point: f32,
    tan_half_fov: vec2<f32>,
    device_tier: u32,
    deterministic_rendering: u32,
    framebuffer_resolution: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> frame: FrameUniformBuffer;

// ---- Custom bind group (group 2) ----

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

// ---- Vertex output from instanced_mesh.wgsl ----

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) texcoord: vec2<f32>,
    @location(2) normal_world_space: vec3<f32>,
    @location(3) @interpolate(flat) additive_tint_rgba: vec4<f32>,
    @location(4) @interpolate(flat) outline_mask_ids: vec2<u32>,
    @location(5) @interpolate(flat) picking_layer_id: vec4<u32>,
    @location(6) position_world: vec3<f32>,
    @location(7) position_object: vec3<f32>,
};

// ---- Ray-AABB intersection ----

// Intersect a ray with the [0,1]^3 axis-aligned bounding box.
// Returns (t_near, t_far). If t_near > t_far, the ray misses the box.
fn intersect_aabb(origin: vec3<f32>, dir: vec3<f32>) -> vec2<f32> {
    let inv_dir = 1.0 / dir;
    let t0 = (vec3<f32>(0.0) - origin) * inv_dir;
    let t1 = (vec3<f32>(1.0) - origin) * inv_dir;
    let t_min = min(t0, t1);
    let t_max = max(t0, t1);
    let t_near = max(max(t_min.x, t_min.y), t_min.z);
    let t_far = min(min(t_max.x, t_max.y), t_max.z);
    return vec2<f32>(t_near, t_far);
}

// ---- Camera ray computation ----

fn camera_ray_direction(frag_coord: vec2<f32>) -> vec3<f32> {
    let screen_uv = frag_coord / frame.framebuffer_resolution;

    // Orthographic camera: all rays are parallel
    if frame.tan_half_fov.x >= 3.402823e+38 {
        return frame.camera_forward;
    }

    // Perspective camera: compute per-pixel ray direction
    let ndc = vec2<f32>(screen_uv.x - 0.5, 0.5 - screen_uv.y) * 2.0;
    let view_dir = vec3<f32>(ndc * frame.tan_half_fov, -1.0);

    // Transform from view to world space.
    // Since view_from_world is orthonormal, right-multiply gives the inverse (transpose).
    let world_dir = vec3<f32>(
        dot(view_dir, vec3<f32>(frame.view_from_world[0].x, frame.view_from_world[1].x, frame.view_from_world[2].x)),
        dot(view_dir, vec3<f32>(frame.view_from_world[0].y, frame.view_from_world[1].y, frame.view_from_world[2].y)),
        dot(view_dir, vec3<f32>(frame.view_from_world[0].z, frame.view_from_world[1].z, frame.view_from_world[2].z)),
    );

    return normalize(world_dir);
}

// ---- Entry point ----

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // The object-space position maps directly to [0,1]^3 volume coordinates.
    let surface_pos = in.position_object;

    // Compute camera ray direction in world space.
    // For the DICOM demo the mesh is axis-aligned, so world-space ≈ object-space direction.
    let ray_dir_world = camera_ray_direction(in.position.xy);

    // Intersect the camera ray with the [0,1]^3 volume AABB.
    // We back up the ray origin so we can compute the full entry/exit.
    let ray_origin = surface_pos - ray_dir_world * 2.0;
    let hit = intersect_aabb(ray_origin, ray_dir_world);

    if hit.x > hit.y || hit.y < 0.0 {
        discard;
    }

    let t_near = max(hit.x, 0.0);
    let t_far = hit.y;
    let entry = ray_origin + ray_dir_world * t_near;
    let exit = ray_origin + ray_dir_world * t_far;
    let march_length = distance(entry, exit);
    let march_dir = normalize(exit - entry);

    let num_steps: i32 = 128;
    let step_size: f32 = march_length / f32(num_steps);

    var accumulated_color = vec3<f32>(0.0);
    var accumulated_alpha: f32 = 0.0;

    let range_min = params.value_range.x;
    let range_max = params.value_range.y;
    let range_extent = max(range_max - range_min, 0.001);

    let tex_dim = textureDimensions(volume_texture);

    for (var i: i32 = 0; i < num_steps; i = i + 1) {
        if accumulated_alpha >= 0.95 {
            break;
        }

        let t = f32(i) * step_size;
        let sample_pos = entry + march_dir * t;

        // Skip samples outside the [0,1]^3 volume.
        if any(sample_pos < vec3<f32>(0.0)) || any(sample_pos > vec3<f32>(1.0)) {
            continue;
        }

        // Convert normalized [0,1] coordinates to texel coordinates.
        let texel = vec3<i32>(
            i32(sample_pos.x * f32(tex_dim.x)),
            i32(sample_pos.y * f32(tex_dim.y)),
            i32(sample_pos.z * f32(tex_dim.z)),
        );
        let clamped = clamp(texel, vec3<i32>(0), vec3<i32>(tex_dim) - vec3<i32>(1));

        // Load the texel directly (R32Float is not filterable).
        let raw_value = textureLoad(volume_texture, clamped, 0).r;

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

// Cool-to-warm colormap transfer function for medical data.
fn transfer_function(intensity: f32) -> vec3<f32> {
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
