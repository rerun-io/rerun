#import <../global_bindings.wgsl>
#import <../types.wgsl>

fn apply_depth_offset(position: Vec4, offset: f32) -> Vec4 {
    // TODO: doc
    return Vec4(
        position.xy,
        position.z + offset * frame.depth_offset_factor * position.w,
        position.w
    );
}
