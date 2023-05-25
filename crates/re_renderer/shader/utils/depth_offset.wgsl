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

The code in apply_depth_offset acts on the *_proj values by adding a scale multiplier on `w_proj` resulting in:
    x_ndc: x_proj / (-z * w_scale)
    y_ndc: y_proj / (-z * w_scale)
    z_ndc: z_proj / (-z * w_scale)


On GLES/WebGL, the NDC clipspace range for depth is from -1 to 1 and y is flipped.
wgpu/Naga counteracts this by patching all vertex shaders with:
"gl_Position.yz = vec2(-gl_Position.y, gl_Position.z * 2.0 - gl_Position.w);"
Meaning projected coordinates (without any offset) become:

    x_proj_gl: x_proj,
    y_proj_gl: -y_proj,
    z_proj_gl: z_proj * 2 - w_proj,
    w_proj_gl: w_proj

For NDC follows:

    x_ndc: x_proj / w_proj                  = x * f / aspect_ratio / -z
    y_ndc: -y_proj / w_proj                 = -y * f / -z
    z_ndc: (z_proj * 2 - w_proj) / w_proj   = (w * z_near * 2 + z) / -z

Which means depth precision is greatly reduced before hitting the depth buffer
and then further by shifting back to the [0, 1] range in which depth is stored.

This is a general issue, not specific to our depth offset implementation, affecting precision for all depth values.

Note that for convenience we still use inverse depth (otherwise we'd have to flip all depth tests),
but this does actually neither improve not worsen precision, in any case most of the precision is
somewhere in the middle of the depth range (see also https://developer.nvidia.com/content/depth-precision-visualized).

The only reliable ways to mitigate this we found so far are:
* higher near plane distance
* larger depth offset

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

    var w_scale_bias = f32eps * offset;
    if frame.hardware_tier == HARDWARE_TIER_GLES {
        // Empirically determined, see section on GLES above.
        w_scale_bias *= 1000.0;
    }
    let w_scale = 1.0 - w_scale_bias;

    return Vec4(
        position.xyz,
        position.w * w_scale,
    );
}
