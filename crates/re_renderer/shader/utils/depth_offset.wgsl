#import <../global_bindings.wgsl>
#import <../types.wgsl>
#import <../device_info.wgsl>

fn apply_depth_offset(position: Vec4, offset: f32) -> Vec4 {
    // We're using inverse z, i.e. 0.0 is far, 1.0 is near.
    // We want a positive offset to move towards the viewer, so offset needs to be added.
    //
    // With this in place we still may cross over to 0.0 (the far plane) too early,
    // making objects disappear into the far when they'd be otherwise still rendered.
    // Since we're actually supposed to have an *infinite* far plane this should never happen!
    // Therefore we simply dictacte a minimum z value.
    // This ofc wrecks the depth offset and may cause z fighting with all very far away objects, but it's better than having things disappear!

    if true {
        // This path assumes a `f32` depth buffer!

        // 1.0 * eps _should_ be enough, but in practice it causes Z-fighting for unknown reasons.
        // Maybe because of GPU interpolation of vertex coordinates?
        var eps = 5.0 * f32eps;

        if GLES {
            // On GLES/WebGL, the NDC clipspace range for depth is from -1 to 1 and y is flipped.
            // wgpu/Naga counteracts this by patching all vertex shader with:
            // "gl_Position.yz = vec2(-gl_Position.y, gl_Position.z * 2.0 - gl_Position.w);",
            // This is great, since it means that we can pretend depth is 0 to 1 as specified by WebGPU.
            // But it completely messes up depth precision, in particular since we use
            // an inverse depth projection that tries to make use of the high float precision closer to zero.
            eps *= 1000.0;
        }

        return Vec4(
            position.xy,
            max(position.z * (1.0 + eps * offset), f32eps),
            position.w
        );
    } else {
        // Causes Z-collision at far distances
        let eps = f32eps;
        return Vec4(
            position.xy,
            max(position.z + eps * offset * position.w, f32eps),
            position.w
        );
    }
}
