#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

struct UniformBuffer {
    format: u32,
    primaries: u32,
};

@group(0) @binding(0)
var<uniform> uniform_buffer: UniformBuffer;

@group(1) @binding(1)
var input_texture: texture_2d<u32>;


const FORMAT_Y_UV = 0u;
const FORMAT_YUYV16 = 1u;

const PRIMARIES_BT601 = 0u;
const PRIMARIES_BT709 = 1u;


@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {

    switch (uniform_buffer.format)  {
        case FORMAT_Y_UV: {
            return vec4f(0.0, 0.0, 1.0, 1.0);
        }
        case FORMAT_YUYV16: {
            return vec4f(1.0, 0.0, 1.0, 1.0);
        }
        default: {
            // Something went wrong!
            return vec4f(0.0, 0.0, 0.0, 0.0);
        }
    }

    return vec4f(1.0, 0.0, 1.0, 1.0);
}
