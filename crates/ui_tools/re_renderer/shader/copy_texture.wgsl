// Reads the content of a texture and writes it out as is.
//
// This is needed e.g. on WebGL to convert from a depth format to a regular color format that can be read back to the CPU.

#import <./types.wgsl>
#import <./global_bindings.wgsl>
#import <./screen_triangle_vertex.wgsl>

@group(1) @binding(0)
var tex: texture_2d<f32>;

@fragment
fn main(in: FragmentInput) -> @location(0) vec4f {
    return textureSample(tex, nearest_sampler_clamped, in.texcoord);
}
