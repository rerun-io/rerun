#import <./types.wgsl>

struct VertexOutput {
    // Mark output position as invariant so it's safe to use it with depth test Equal.
    // Without @invariant, different usages in different render pipelines might optimize differently,
    // causing slightly different results.
    @invariant @builtin(position) position: Vec4,
    @location(0) texcoord: Vec2,
};
