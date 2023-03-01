#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

@group(0) @binding(0)
var closest_pos_texture: texture_2d<f32>; // TODO: Rename everywhere

struct FrameUniformBuffer {
    step_width: i32,
};
@group(0) @binding(1)
var<uniform> uniforms: FrameUniformBuffer;

fn distance_sq_two_channels(texcoord: Vec2, positions: Vec4) -> Vec2 {
    let a = texcoord - positions.xy;
    let b = texcoord - positions.zw;
    return Vec2(dot(a,a), dot(b,b));
}

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    let resolution = textureDimensions(closest_pos_texture).xy;
    let center_coord = IVec2(Vec2(resolution) * in.texcoord);

    var closest_positions_a = Vec2(-1.0);
    var closest_distance_sq_a = 99999.0;
    var closest_positions_b = Vec2(-1.0);
    var closest_distance_sq_b = 99999.0;

    for (var y: i32 = -1; y <= 1; y += 1) {
        for (var x: i32 = -1; x <= 1; x += 1) {
            let positions = textureLoad(closest_pos_texture, center_coord + IVec2(x, y) * uniforms.step_width, 0);

            let to_pos_a = in.texcoord - positions.xy;
            let distance_sq_a = dot(to_pos_a, to_pos_a);
            if closest_distance_sq_a > distance_sq_a {
                closest_distance_sq_a = distance_sq_a;
                closest_positions_a = positions.xy;
            }

            let to_pos_b = in.texcoord - positions.xy;
            let distance_sq_b = dot(to_pos_b, to_pos_b);
            if closest_distance_sq_b > distance_sq_b {
                closest_distance_sq_b = distance_sq_b;
                closest_positions_b = positions.zw;
            }
        }
    }

    return Vec4(closest_positions_a, closest_positions_b);
}
