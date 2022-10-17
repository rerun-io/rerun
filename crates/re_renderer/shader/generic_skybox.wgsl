struct Locals {
    view_from_world: mat4x4<f32>,
    projection_from_view: mat4x4<f32>,
    projection_from_world: mat4x4<f32>,
    world_from_projection: mat4x4<f32>,
    camera_position: vec3<f32>
};
@group(0) @binding(0)
var<uniform> frame_uniform_buffer: Locals;

fn camera_dir_from_screenuv(texcoord: vec2<f32>) -> vec3<f32> {
    // There's slightly faster ways to do this (e.g. by precomputing where projection-space(1, 1, 0, 1) ends up),
    // but this approach is relatively easy to follow.
    let x = texcoord.x * 2.0 - 1.0;
    let y = (1.0 - texcoord.y) * 2.0 - 1.0;
    let pos_world = frame_uniform_buffer.world_from_projection * vec4<f32>(x, y, 1.0, 1.0);
    return normalize(pos_world.xyz / pos_world.w - frame_uniform_buffer.camera_position);
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texcoord: vec2<f32>,
};

fn skybox_dark(dir: vec3<f32>) -> vec3<f32> {
    let rgb = dir * 0.5 + vec3<f32>(0.5);
    return vec3<f32>(0.05) + 0.20 * rgb;
}

fn skybox_light(dir: vec3<f32>) -> vec3<f32> {
    let rgb = dir * 0.5 + vec3<f32>(0.5);
    return vec3<f32>(0.85) + 0.15 * rgb;
}

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
    // TODO(andreas): implement
    let camera_dir = camera_dir_from_screenuv(in.texcoord);
    let rgb = skybox_dark(camera_dir);
    return vec4<f32>(rgb, 1.0);
}
