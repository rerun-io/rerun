#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./utils/srgb.wgsl>

fn camera_dir_from_screenuv(texcoord: Vec2) -> vec3<f32> {
    // convert [0, 1] to [-1, +1 (Normalized Device Coordinates)
    let ndc = Vec2(texcoord.x - 0.5, 0.5 - texcoord.y) * 2.0;

    // Negative z since z dir is towards viewer (by current RUB convention).
    let view_space_dir = Vec3(ndc * frame.tan_half_fov, -1.0);

    // Note that since view_from_world is an orthonormal matrix, multiplying it from the right
    // means multiplying it with the transpose, meaning multiplying with the inverse!
    // (i.e. we get world_from_view for free as long as we only care about directions!)
    let world_space_dir = (view_space_dir * frame.view_from_world).xyz;

    return normalize(world_space_dir);
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
    let rgb = skybox_dark_srgb(camera_dir); // TODO(andreas): Allow switchting to skybox_light
    return Vec4(linear_from_srgb(rgb), 1.0);
    //return Vec4(camera_dir, 1.0);
}
