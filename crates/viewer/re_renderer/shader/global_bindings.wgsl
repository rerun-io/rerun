struct FrameUniformBuffer {
    view_from_world: mat4x3f,
    projection_from_view: mat4x4f,
    projection_from_world: mat4x4f,

    /// Camera position in world space.
    camera_position: vec3f,

    /// Padding to ensure proper alignment (vec2 needs 8-byte alignment).
    _padding0: f32,

    /// For perspective: Multiply this with a camera distance to get a measure of how wide a pixel is in world units (x and y separately for anamorphic cameras).
    /// For orthographic: This is the world size value, independent of distance.
    /// Note: This appears as vec2f in WGSL but occupies 16 bytes (vec4f space) due to struct layout rules.
    pixel_world_size_from_camera_distance: vec2f,

    /// Explicit padding after vec2f (WGSL vec2f in struct uses 8 bytes but next field aligns to 16).
    _padding_after_pixel_size: vec2f,

    /// Camera direction in world space.
    /// Same as -vec3f(view_from_world[0].z, view_from_world[1].z, view_from_world[2].z)
    camera_forward: vec3f,

    /// How many pixels there are per point.
    /// I.e. the UI zoom factor.
    pixels_from_point: f32,

    /// (tan(fov_y / 2) * aspect_ratio, tan(fov_y /2)), i.e. half ratio of screen dimension to screen distance in x & y.
    /// Both values are set to f32max for orthographic projection
    tan_half_fov: vec2f,

    /// re_renderer defined device tier.
    device_tier: u32,

    /// boolean (0/1): set to true for snapshot tests to minimize
    /// GPU/driver-specific stuff like alpha-to-coverage.
    deterministic_rendering: u32,
};

@group(0) @binding(0)
var<uniform> frame: FrameUniformBuffer;

@group(0) @binding(1)
var nearest_sampler_repeat: sampler;
@group(0) @binding(2)
var nearest_sampler_clamped: sampler;
@group(0) @binding(3)
var trilinear_sampler_repeat: sampler;

// See config.rs#DeviceTier
const DEVICE_TIER_GLES = 0u;
const DEVICE_TIER_WEBGPU = 1u;
