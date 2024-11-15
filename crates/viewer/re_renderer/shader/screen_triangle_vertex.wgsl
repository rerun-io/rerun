#import <./types.wgsl>

struct VertexOutput {
    // Mark output position as invariant so it's safe to use it with depth test Equal.
    // Without @invariant, different usages in different render pipelines might optimize differently,
    // causing slightly different results.
    @invariant @builtin(position)
    position: vec4f,
    @location(0)
    texcoord: vec2f,
};

// Workaround for https://github.com/gfx-rs/naga/issues/2252
// Naga emits invariant flag on fragment input, but some implementations don't allow this.
// Therefore we drop position here (we could still pass it in if needed if we drop the invariant flag)
struct FragmentInput {
    @location(0) texcoord: vec2f,
};
