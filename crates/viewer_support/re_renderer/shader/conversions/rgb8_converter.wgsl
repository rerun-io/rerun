#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

struct UniformBuffer {
    target_texture_size: vec2u,
    is_bgr: u32,
    _padding: u32,
};

@group(0) @binding(0)
var<uniform> uniform_buffer: UniformBuffer;

@group(0) @binding(1)
var input_texture: texture_2d<u32>;

fn unpack_rgb(coords: vec2u) -> vec3u {
    let byte_index = coords.x * 3u;
    let texel_index = byte_index / 4u;
    let byte_offset = byte_index % 4u;

    let a = textureLoad(input_texture, vec2u(texel_index, coords.y), 0);
    var rgb: vec3u;

    switch byte_offset {
        case 0u: {
            rgb = a.rgb;
        }
        case 1u: {
            rgb = a.gba;
        }
        case 2u: {
            let b = textureLoad(input_texture, vec2u(texel_index + 1u, coords.y), 0);
            rgb = vec3u(a.b, a.a, b.r);
        }
        default: {
            let b = textureLoad(input_texture, vec2u(texel_index + 1u, coords.y), 0);
            rgb = vec3u(a.a, b.r, b.g);
        }
    }

    if uniform_buffer.is_bgr != 0u {
        rgb = rgb.bgr;
    }

    return rgb;
}

@fragment
fn fs_main(in: FragmentInput) -> @location(0) vec4f {
    let coords = vec2u(vec2f(uniform_buffer.target_texture_size) * in.texcoord);
    let rgb = unpack_rgb(coords);
    return vec4f(vec3f(rgb) / 255.0, 0.0);
}
