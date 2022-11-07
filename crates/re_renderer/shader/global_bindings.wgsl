struct FrameUniformBuffer {
    view_from_world: mat4x3<f32>,
    projection_from_view: mat4x4<f32>,
    projection_from_world: mat4x4<f32>,

    camera_position: vec3<f32>,
    top_right_screen_corner_in_view: vec2<f32>,
    /// Multiply this with a camera distance to get a measure of how wide a pixel is in world units.
    pixel_world_size_from_camera_distance: f32,
};
@group(0) @binding(0)
var<uniform> frame: FrameUniformBuffer;

@group(0) @binding(1)
var nearest_sampler: sampler;

@group(0) @binding(2)
var trilinear_sampler: sampler;
