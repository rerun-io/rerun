#import <./global_bindings.wgsl>

fn camera_dir_from_screenuv(texcoord: vec2<f32>) -> vec3<f32> {
    let x = texcoord.x * 2.0 - 1.0;
    let y = (1.0 - texcoord.y) * 2.0 - 1.0;
    let dir_view = normalize(vec3<f32>(frame.top_right_screen_corner_in_view * vec2<f32>(x, y), 1.0));
    // the inner 3x3 part of the view_from_world matrix is orthonormal
    // A transpose / multiply from right is therefore its inverse!
    return (dir_view * frame.view_from_world).xyz;
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
    let camera_dir = camera_dir_from_screenuv(in.texcoord);
    let rgb = skybox_dark(camera_dir); // TODO(andreas): Allow switchting to skybox_light
    return vec4<f32>(rgb, 1.0);
}
