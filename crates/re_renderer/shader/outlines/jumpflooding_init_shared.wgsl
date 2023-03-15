#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

fn compute_pixel_coords(center_coord: IVec2, unnormalized_edge_pos_a_and_b: Vec4, num_edges_a_and_b: Vec2) -> Vec4 {
    // Normalize edges ans get range from [0, 1] to [-0.5, 0.5].
    let edge_pos_a_and_b = unnormalized_edge_pos_a_and_b / num_edges_a_and_b.xxyy - Vec4(0.5);

    // We're outputting pixel coordinates (0-res) instead of texture coordinates (0-1).
    // This way we don't need to correct for aspect ratio when comparing distances in the jumpflooding steps.
    // When computing the actual outlines themselves we're also interested in pixel distances, not texcoord distances.

    var pixel_coord_a: Vec2;
    if num_edges_a_and_b.x == 0.0 {
        pixel_coord_a = Vec2(f32max);
    } else {
        pixel_coord_a = Vec2(center_coord) + edge_pos_a_and_b.xy;
    }
    var pixel_coord_b: Vec2;
    if num_edges_a_and_b.y == 0.0 {
        pixel_coord_b = Vec2(f32max);
    } else {
        pixel_coord_b = Vec2(center_coord) + edge_pos_a_and_b.zw;
    }

    return Vec4(pixel_coord_a, pixel_coord_b);
}
