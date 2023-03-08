#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

@group(0) @binding(0)
var mask_texture: texture_multisampled_2d<u32>;

fn has_edge(closest_center_sample: UVec2, sample_coord: IVec2, sample_idx: i32) -> Vec2 {
    let mask_neighbor = textureLoad(mask_texture, sample_coord, sample_idx).xy;
    return Vec2(closest_center_sample != mask_neighbor);
}

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    let resolution = textureDimensions(mask_texture).xy;

    // Determine *where* in texture coordinates (with sub-pixel accuracy!) the closest edge to the center is.
    //
    // In Ben Golus article on line rendering (https://bgolus.medium.com/the-quest-for-very-wide-outlines-ba82ed442cd9),
    // anti-aliasing was achieved by a kind of sobel filter on an already resolved target.
    // In our case however, we have a number of different masks, identified by an index per-pixel.
    // Therefore, there is no straight-forward way to resolve this MSAA texture!
    // Resolving accurate sub-pixel edges therefore requires us to look at the sub-samples of the MSAA mask directly.
    //
    // There's a bunch of ways on how to go about this and it's not exactly clear where the trade-offs between quality & performance are.
    // But I found that by using our knowledge of the sampling pattern
    // we can detect the closest edges to each sample, and therefore get a pretty good result with *relatively* few texture fetches.
    //
    // We do so by checking particular edges, summing top their sub-sample positions and dividing by the number of edges.
    //
    //
    // About the sampling pattern:
    // Vulkan: https://registry.khronos.org/vulkan/specs/1.3-khr-extensions/html/chap25.html#primsrast-multisampling
    // Metal: https://developer.apple.com/documentation/metal/mtldevice/2866120-getdefaultsamplepositions
    // DX12 does *not* specify the sampling pattern. However DX11 does, again the same for 4 samples:
    // https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ne-d3d11-d3d11_standard_multisample_quality_levels
    //
    // (0, 0) _____________
    //       |    0       |
    //       |          1 |
    //       | 2          |
    //       |        3   |
    //        ‾‾‾‾‾‾‾‾‾‾‾‾(1, 1)
    //
    // var<private> subsample_positions: array<Vec2, 4> = array<Vec2, 4>(
    //     Vec2(0.375, 0.125),
    //     Vec2(0.875, 0.375),
    //     Vec2(0.125, 0.625),
    //     Vec2(0.625, 0.875)
    // );
    //
    // Note that the algorithm should still produce _some_ edges if this is not the case!

    //let num_samples = textureNumSamples(mask_texture);
    // TODO(andreas): Should we assert somehow on textureNumSamples here?

    let center_coord = IVec2(Vec2(resolution) * in.texcoord);
    let mask_top_left = textureLoad(mask_texture, center_coord, 0).xy;
    let mask_right_top = textureLoad(mask_texture, center_coord, 1).xy;
    let mask_left_bottom = textureLoad(mask_texture, center_coord, 2).xy;
    let mask_bottom_right = textureLoad(mask_texture, center_coord, 3).xy;

    var edge_pos_a_and_b = Vec4(0.0);
    var num_edges_and_b = Vec2(0.0);

    // Internal samples accross the center point
    // Tried weighting this higher, didn't make a difference in quality since we almost always have only a single edge.
    {
        let has_edge = Vec2(mask_top_left != mask_bottom_right) + Vec2(mask_right_top != mask_left_bottom);
        num_edges_and_b += has_edge;
        edge_pos_a_and_b += has_edge.xxyy * 0.5;
    }

    // A lot of this code is repetetive, but wgsl/naga doesn't know yet how to do static indexing from unrolled loops.

    // Sample closest neighbors top/bottom/left/right
    { // right
        let has_edge = has_edge(mask_right_top, center_coord + IVec2(1, 0), 2);
        let edge_pos = Vec2(1.0, 0.5);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }
    { // bottom
        let has_edge = has_edge(mask_bottom_right, center_coord + IVec2(0, 1), 0);
        let edge_pos = Vec2(0.5, 1.0);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }
    { // left
        let has_edge = has_edge(mask_left_bottom, center_coord + IVec2(-1, 0), 1);
        let edge_pos = Vec2(0.0, 0.5);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }
    { // top
        let has_edge = has_edge(mask_top_left, center_coord + IVec2(0, -1), 3);
        let edge_pos = Vec2(0.5, 0.0);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }

    // Sample closest neighbors diagonally.
    // This is not strictly necessary, but empirically the result looks a lot better!
    { // top-right
        let has_edge = has_edge(mask_right_top, center_coord + IVec2(1, -1), 2);
        let edge_pos = Vec2(1.0, 0.0);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }
    { // bottom-right
        let has_edge = has_edge(mask_bottom_right, center_coord + IVec2(1, 1), 0);
        let edge_pos = Vec2(1.0, 1.0);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }
    { // bottom-left
        let has_edge = has_edge(mask_left_bottom, center_coord + IVec2(-1, 1), 1);
        let edge_pos = Vec2(0.0, 1.0);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }
    { // top-left
        let has_edge = has_edge(mask_top_left, center_coord + IVec2(-1, -1), 3);
        //let edge_pos = Vec2(0.0, 0.0);
        //edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }

    // Normalize edges ans get range from [0, 1] to [-0.5, 0.5].
    edge_pos_a_and_b = edge_pos_a_and_b / num_edges_and_b.xxyy - Vec4(0.5);

    var pixel_coord_a: Vec2;
    if num_edges_and_b.x == 0.0 {
        pixel_coord_a = Vec2(-10.0);
    } else {
        pixel_coord_a = Vec2(center_coord) + edge_pos_a_and_b.xy;
    }
    var pixel_coord_b: Vec2;
    if num_edges_and_b.y == 0.0 {
        pixel_coord_b = Vec2(-10.0);
    } else {
        pixel_coord_b = Vec2(center_coord) + edge_pos_a_and_b.zw;
    }

    return Vec4(pixel_coord_a, pixel_coord_b);
}
