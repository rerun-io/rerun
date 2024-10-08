#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    return vec4f(1.0, 0.0, 1.0, 1.0);
}
