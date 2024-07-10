#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

@group(0) @binding(0)
var voronoi_texture: texture_2d<f32>;
@group(0) @binding(1)
var voronoi_sampler: sampler;

struct FrameUniformBuffer {
    step_width: i32,
    // There is actually more padding here. We're only putting this to satisfy lack of
    // wgt::DownlevelFlags::BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED
    padding: vec3i,
};
@group(0) @binding(2)
var<uniform> uniforms: FrameUniformBuffer;


@fragment
fn main(in: FragmentInput) -> @location(0) vec4f {
    let resolution = vec2f(textureDimensions(voronoi_texture).xy);
    let pixel_step = vec2f(f32(uniforms.step_width), f32(uniforms.step_width)) / resolution;
    let pixel_coordinates = floor(resolution * in.texcoord);

    var closest_positions_a = vec2f(f32min);
    var closest_distance_sq_a = f32max;
    var closest_positions_b = vec2f(f32min);
    var closest_distance_sq_b = f32max;

    for (var y: i32 = -1; y <= 1; y += 1) {
        for (var x: i32 = -1; x <= 1; x += 1) {
            let texcoord = in.texcoord + vec2f(f32(x), f32(y)) * pixel_step;
            let positions_a_and_b = textureSampleLevel(voronoi_texture, voronoi_sampler, texcoord, 0.0);
            let to_positions_a_and_b = positions_a_and_b - pixel_coordinates.xyxy;

            let distance_sq_a = dot(to_positions_a_and_b.xy, to_positions_a_and_b.xy);
            if closest_distance_sq_a > distance_sq_a {
                closest_distance_sq_a = distance_sq_a;
                closest_positions_a = positions_a_and_b.xy;
            }

            let distance_sq_b = dot(to_positions_a_and_b.zw, to_positions_a_and_b.zw);
            if closest_distance_sq_b > distance_sq_b {
                closest_distance_sq_b = distance_sq_b;
                closest_positions_b = positions_a_and_b.zw;
            }
        }
    }

    return vec4f(closest_positions_a, closest_positions_b);
}
