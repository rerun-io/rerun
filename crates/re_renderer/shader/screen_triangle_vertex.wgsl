#import <./types.wgsl>

struct VertexOutput {
    // Mark output position as invariant so it's safe to use it with depth test Equal.
    // Without @invariant, different usages in different render pipelines might optimize differently,
    // causing slightly different results.
    //
    // TODO(andreas): Chrome/Tint does not support `@invariant`
    // https://bugs.chromium.org/p/chromium/issues/detail?id=1439273
    //@invariant
    @builtin(position)
    position: Vec4,
    @location(0)
    texcoord: Vec2,
};

// Workaround for https://github.com/gfx-rs/naga/issues/2252
// Naga emits invariant flag on fragment input, but some implementations don't allow this.
// Therefore we drop position here (we could still pass it in if needed if we drop the invariant flag)
struct FragmentInput {
    @location(0) texcoord: Vec2,
};
