#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./utils/srgb.wgsl>
#import <./utils/camera.wgsl>
#import <./screen_triangle_vertex.wgsl>

fn skybox_dark_srgb(dir: vec3f) -> vec3f {
    let rgb = dir * 0.5 + vec3f(0.5);
    return vec3f(0.05) + 0.20 * rgb;
}

fn skybox_light_srgb(dir: vec3f) -> vec3f {
    let rgb = dir * 0.5 + vec3f(0.5);
    return vec3f(0.85) + 0.15 * rgb;
}

@fragment
fn main(in: FragmentInput) -> @location(0) vec4f {
    let camera_dir = camera_ray_direction_from_screenuv(in.texcoord);
    // Messing with direction a bit so it looks like in our old three-d based renderer (for easier comparison)
    let rgb = skybox_dark_srgb(camera_dir); // TODO(andreas): Allow switching to skybox_light
    return vec4f(linear_from_srgb(rgb), 1.0);
    //return vec4f(camera_dir, 1.0);
}
