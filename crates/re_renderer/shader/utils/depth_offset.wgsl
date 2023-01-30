#import <../global_bindings.wgsl>
#import <../types.wgsl>

fn apply_depth_offset(position: Vec4, offset: f32) -> Vec4 {
    // Z buffer z is computed using position.z/position.w,
    // Therefore, to affect the final output by a given offset we need to multiply it with position.w.
    // This also means though that we're loosing a lot of precision
    //
    // We're using inverse z, i.e. 0.0 is far, 1.0 is near.
    // We want a positive depth_offset_factor to move towards the viewer, so offset needs to be added.
    //
    // With this in place we still may cross over to 0.0 (the far plane) too early,
    // making objects disappear into the far when they'd be otherwise stilil rendered.
    // Since we're actually supposed to have an *infinite* far plane this should never happen!
    // Therefore we simply dictacte a minimum z value.
    // This ofc wrecks the depth offset and may cause z fighting with all very far away objects, but it's better than having things disappear!
    return Vec4(
        position.xy,
        max(position.z + frame.depth_offset_factor * offset * position.w, f32eps),
        position.w
    );
}
