#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

@group(0) @binding(0)
var closest_pos_texture: texture_2d<f32>; // TODO: Rename everywhere

struct FrameUniformBuffer {
    step_width: i32,
};
@group(0) @binding(1)
var<uniform> uniforms: FrameUniformBuffer;

fn distance_sq(pos0: Vec2, pos1: Vec2) -> f32 {
    let to = pos0 - pos1;
    return dot(to, to);
}

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    let resolution = Vec2(textureDimensions(closest_pos_texture).xy);
    let center_coord_f = Vec2(resolution) * in.texcoord;
    let center_coord = IVec2(center_coord_f);

    var closest_positions_a = Vec2(-1.0);
    var closest_distance_sq_a = 99999.0;
    var closest_positions_b = Vec2(-1.0);
    var closest_distance_sq_b = 99999.0;

    for (var y: i32 = -1; y <= 1; y += 1) {
        for (var x: i32 = -1; x <= 1; x += 1) {
            let positions = textureLoad(closest_pos_texture, center_coord + IVec2(x, y) * uniforms.step_width, 0);
            let pos_a = positions.xy;
            let pos_b = positions.zw;

            let distance_sq_a = distance_sq(pos_a * resolution, center_coord_f);
            if closest_distance_sq_a > distance_sq_a {
                closest_distance_sq_a = distance_sq_a;
                closest_positions_a = pos_a;
            }

            let distance_sq_b = distance_sq(pos_b * resolution, center_coord_f);
            if closest_distance_sq_b > distance_sq_b {
                closest_distance_sq_b = distance_sq_b;
                closest_positions_b = pos_b;
            }
        }
    }

    return Vec4(closest_positions_a, closest_positions_b);
}
