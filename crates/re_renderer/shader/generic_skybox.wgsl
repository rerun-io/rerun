#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./utils/srgb.wgsl>
#import <./utils/camera.wgsl>

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
    let rgb = skybox_dark_srgb(camera_dir); // TODO(andreas): Allow switchting to skybox_light
    return Vec4(linear_from_srgb(rgb), 1.0);
    //return Vec4(camera_dir, 1.0);
}
