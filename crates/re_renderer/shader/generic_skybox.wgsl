#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./utils/srgb.wgsl>

fn camera_dir_from_screenuv(texcoord: Vec2) -> vec3<f32> {
    let x = texcoord.x * 2.0 - 1.0;
    let y = (1.0 - texcoord.y) * 2.0 - 1.0;
    let dir_view = normalize(Vec3(frame.top_right_screen_corner_in_view * Vec2(x, y), 1.0));
    // the inner 3x3 part of the view_from_world matrix is orthonormal
    // A transpose / multiply from right is therefore its inverse!
    return (dir_view * frame.view_from_world).xyz;
}

struct VertexOutput {
    @builtin(position) position: Vec4,
    @location(0) texcoord: Vec2,
};

fn skybox_dark_srgb(dir: Vec3) -> Vec3 {
    let rgb = dir * 0.5 + vec3<f32>(0.5);
    return vec3<f32>(0.05) + 0.20 * rgb;
}

fn skybox_light_srgb(dir: Vec3) -> Vec3 {
    let rgb = dir * 0.5 + vec3<f32>(0.5);
    return vec3<f32>(0.85) + 0.15 * rgb;
}

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    let camera_dir = camera_dir_from_screenuv(in.texcoord);
    // Messing with direction a bit so it looks like in our old three-d based renderer (for easier comparision)
    let rgb = skybox_dark_srgb(Vec3(1.0 - camera_dir.y, -camera_dir.x, -camera_dir.z)); // TODO(andreas): Allow switchting to skybox_light
    return Vec4(linear_from_srgb(rgb), 1.0);
}
