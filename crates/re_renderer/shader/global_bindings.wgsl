struct FrameUniformBuffer {
    view_from_world: mat4x3<f32>,
    projection_from_view: mat4x4<f32>,
    projection_from_world: mat4x4<f32>,

    /// Camera position in world space.
    camera_position: vec3<f32>,

    /// For perspective: Multiply this with a camera distance to get a measure of how wide a pixel is in world units.
    /// For orthographic: This is the world size value, independent of distance.
    pixel_world_size_from_camera_distance: f32,

    /// Camera direction in world space.
    /// Same as -Vec3(view_from_world[0].z, view_from_world[1].z, view_from_world[2].z)
    camera_forward: vec3<f32>,

    /// How many pixels there are per point.
    /// I.e. the ui scaling factor.
    pixels_from_point: f32,

    /// (tan(fov_y / 2) * aspect_ratio, tan(fov_y /2)), i.e. half ratio of screen dimension to screen distance in x & y.
    /// Both values are set to positive infinity for orthographic projection
    tan_half_fov: vec2<f32>,

    // Size used for all sizes given with Size::AUTO, may be both a point or a world size.
    auto_size: f32,

    // Size used for all sizes given with Size::AUTO_LARGE, may be both a point or a world size.
    auto_size_large: f32,

    /// Factor used to compute depth offsets, see `depth_offset.wgsl`.
    depth_offset_factor: f32,
};
@group(0) @binding(0)
var<uniform> frame: FrameUniformBuffer;

@group(0) @binding(1)
var nearest_sampler: sampler;

@group(0) @binding(2)
var trilinear_sampler: sampler;
