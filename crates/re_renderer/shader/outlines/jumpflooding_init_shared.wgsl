#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

fn compute_pixel_coords(center_coord: vec2i, unnormalized_edge_pos_a_and_b: vec4f, num_edges_a_and_b: vec2f) -> vec4f {
    // Normalize edges ans get range from [0, 1] to [-0.5, 0.5].
    let edge_pos_a_and_b = unnormalized_edge_pos_a_and_b / num_edges_a_and_b.xxyy - vec4f(0.5);

    // We're outputting pixel coordinates (0-res) instead of texture coordinates (0-1).
    // This way we don't need to correct for aspect ratio when comparing distances in the jumpflooding steps.
    // When computing the actual outlines themselves we're also interested in pixel distances, not texcoord distances.

    var pixel_coord_a: vec2f;
    if num_edges_a_and_b.x == 0.0 {
        pixel_coord_a = vec2f(f32max);
    } else {
        pixel_coord_a = vec2f(center_coord) + edge_pos_a_and_b.xy;
    }
    var pixel_coord_b: vec2f;
    if num_edges_a_and_b.y == 0.0 {
        pixel_coord_b = vec2f(f32max);
    } else {
        pixel_coord_b = vec2f(center_coord) + edge_pos_a_and_b.zw;
    }

    return vec4f(pixel_coord_a, pixel_coord_b);
}
