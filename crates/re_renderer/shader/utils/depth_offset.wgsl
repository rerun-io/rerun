#import <../global_bindings.wgsl>
#import <../types.wgsl>

/*
We use reverse infinite depth, as promoted by https://developer.nvidia.com/content/depth-precision-visualized

The projection matrix (from `glam::Mat4::perspective_infinite_reverse_rh`) looks like this:

f / aspect_ratio   0     0      0
0                  f     0      0
0                  0     0      z_near
0                  0    -1      0

This means after multiplication with xyzw (with w=1) we end up with:

    x_proj: x * f / aspect_ratio,
    y_proj: y * f,
    z_proj: w * z_near,
    w_proj: -z

This is then projected by dividing with w, giving:

    x_ndc: x_proj / w_proj
    y_ndc: y_proj / w_proj
    z_ndc: z_proj / w_proj

Without z offset, we get this:

    x_ndc: x * f / aspect_ratio / -z
    y_ndc: y * f / -z
    z_ndc: w * z_near / -z

The negative -z axis is away from the camera, so with w=1 we get
z_near mapping to z_ndc=1, and infinity mapping to z_ndc=0.

The code below act on the *_proj values by adding a scale multiplier on `w_proj` resulting in:
    x_ndc: x_proj / (-z * w_scale)
    y_ndc: y_proj / (-z * w_scale)
    z_ndc: z_proj / (-z * w_scale)
*/

fn apply_depth_offset(position: Vec4, offset: f32) -> Vec4 {
    // On GLES/WebGL, the NDC clipspace range for depth is from -1 to 1 and y is flipped.
    // wgpu/Naga counteracts this by patching all vertex shaders with:
    // "gl_Position.yz = vec2(-gl_Position.y, gl_Position.z * 2.0 - gl_Position.w);",
    // This doesn't matter for us though.

    // This path assumes a `f32` depth buffer.

    // We set up the depth comparison to Greater, so that large z means closer (overdraw).
    // We want a greater offset to win over a smaller offset,
    // so a great depth offset should result in a large z_ndc.
    // How do we get there? We let large depth offset lead to a smaller divisor (w_proj):

    return Vec4(
        position.xyz,
        position.w * (1.0 - f32eps * offset),
    );
}
