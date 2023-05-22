struct FrameUniformBuffer {
    view_from_world: Mat4x3,
    projection_from_view: Mat4,
    projection_from_world: Mat4,

    /// Camera position in world space.
    camera_position: Vec3,

    /// For perspective: Multiply this with a camera distance to get a measure of how wide a pixel is in world units.
    /// For orthographic: This is the world size value, independent of distance.
    pixel_world_size_from_camera_distance: f32,

    /// Camera direction in world space.
    /// Same as -Vec3(view_from_world[0].z, view_from_world[1].z, view_from_world[2].z)
    camera_forward: Vec3,

    /// How many pixels there are per point.
    /// I.e. the ui scaling factor.
    pixels_from_point: f32,

    /// (tan(fov_y / 2) * aspect_ratio, tan(fov_y /2)), i.e. half ratio of screen dimension to screen distance in x & y.
    /// Both values are set to f32max for orthographic projection
    tan_half_fov: Vec2,

    // Size used for all point radii given with Size::AUTO.
    auto_size_points: f32,

    // Size used for all line radii given with Size::AUTO.
    auto_size_lines: f32,

    /// re_renderer defined hardware tier.
    hardware_tier: u32,
};

@group(0) @binding(0)
var<uniform> frame: FrameUniformBuffer;

@group(0) @binding(1)
var nearest_sampler: sampler;

@group(0) @binding(2)
var trilinear_sampler: sampler;

// See config.rs#HardwareTier
const HARDWARE_TIER_GLES = 0u;
const HARDWARE_TIER_WEBGPU = 1u;
