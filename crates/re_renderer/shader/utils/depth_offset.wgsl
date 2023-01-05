#import <../global_bindings.wgsl>
#import <../types.wgsl>

fn apply_depth_offset(position: Vec4, offset: f32) -> Vec4 {
    // Z buffer z is computed using position.z/position.w,
    // Therefore, to affect the final output by a given offset we need to multiply it with position.w.
    // (This also means that we're loosing some precision!)
    //
    // We're using inverse z, i.e. 0.0 is far, 1.0 is near.
    // We want a positive depth_offset_factor to move towards the viewer, so offset needs to be added.
    return Vec4(
        position.xy,
        position.z + frame.depth_offset_factor * offset * position.w,
        position.w
    );
}
