struct FrameUniformBuffer {
    view_from_world: mat4x3<f32>,
    projection_from_view: mat4x4<f32>,
    projection_from_world: mat4x4<f32>,

    camera_position: vec3<f32>,
    /// (tan(fov_y / 2) * aspect_ratio, tan(fov_y /2)), i.e. half ratio of screen dimension to screen distance in x & y.
    tan_half_fov: vec2<f32>,
    /// Multiply this with a camera distance to get a measure of how wide a pixel is in world units.
    pixel_world_size_from_camera_distance: f32,
};
@group(0) @binding(0)
var<uniform> frame: FrameUniformBuffer;

@group(0) @binding(1)
var nearest_sampler: sampler;

@group(0) @binding(2)
var trilinear_sampler: sampler;
