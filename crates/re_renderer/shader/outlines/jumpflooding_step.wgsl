#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

@group(0) @binding(0)
var voronoi_texture: texture_2d<f32>; // TODO: Rename everywhere

struct FrameUniformBuffer {
    step_width: i32,
};
@group(0) @binding(1)
var<uniform> uniforms: FrameUniformBuffer;

fn length_sq_aspect_ratio_corrected(v: Vec2, aspect_ratio: f32) -> f32 {
    return v.x * v.x * aspect_ratio * aspect_ratio + v.y * v.y;
}

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    let resolution = Vec2(textureDimensions(voronoi_texture).xy);
    let aspect_ratio = resolution.x / resolution.y;
    let center_coord = IVec2(resolution * in.texcoord);

    var closest_positions_a = Vec2(-99.0);
    var closest_distance_sq_a = 99999.0;
    var closest_positions_b = Vec2(-99.0);
    var closest_distance_sq_b = 99999.0;

    // TODO: Ignore position at 0,0

    for (var y: i32 = -1; y <= 1; y += 1) {
        for (var x: i32 = -1; x <= 1; x += 1) {
            let positions_a_and_b = textureLoad(voronoi_texture, center_coord + IVec2(x, y) * uniforms.step_width, 0);
            let to_positions_a_and_b = positions_a_and_b - Vec4(in.texcoord, in.texcoord);

            let distance_sq_a = length_sq_aspect_ratio_corrected(to_positions_a_and_b.xy, aspect_ratio);
            if closest_distance_sq_a > distance_sq_a {
                closest_distance_sq_a = distance_sq_a;
                closest_positions_a = positions_a_and_b.xy;
            }

            let distance_sq_b = length_sq_aspect_ratio_corrected(to_positions_a_and_b.zw, aspect_ratio);
            if closest_distance_sq_b > distance_sq_b {
                closest_distance_sq_b = distance_sq_b;
                closest_positions_b = positions_a_and_b.zw;
            }
        }
    }

    return Vec4(closest_positions_a, closest_positions_b);
    // Useful for debugging:
    //return textureLoad(voronoi_texture, center_coord, 0);
}
