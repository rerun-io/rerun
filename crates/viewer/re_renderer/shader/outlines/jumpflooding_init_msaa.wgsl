#import <jumpflooding_init_shared.wgsl>

@group(0) @binding(0)
var mask_texture: texture_multisampled_2d<u32>;

fn has_edge(max_coord: vec2i, closest_center_sample: vec2u, sample_coord: vec2i, sample_idx: i32) -> vec2f {
    // Note that `textureLoad` calls with out-of-bounds coordinates are allowed to return *any*
    // value on the texture or "transparent"/"opaque" zero.
    // See https://www.w3.org/TR/WGSL/#textureload
    // Therefore, if we want consistent behavior, we have to do the clamp ourselves.
    let clamped_coord = clamp(sample_coord, vec2i(0), max_coord);
    let mask_neighbor = textureLoad(mask_texture, clamped_coord, sample_idx).xy;
    return vec2f(closest_center_sample != mask_neighbor);
}


// Determine *where* in texture coordinates (with sub-pixel accuracy!) the closest edge to the center is.
//
// In Ben Golus article on line rendering (https://bgolus.medium.com/the-quest-for-very-wide-outlines-ba82ed442cd9),
// anti-aliasing was achieved by a kind of sobel filter on an already resolved target.
// In our case however, we have a number of different masks, identified by an index per-pixel.
// Therefore, there is no straight-forward way to resolve this MSAA texture!
// Resolving accurate sub-pixel edges requires us to look at the sub-samples of the MSAA mask directly.
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
// var<private> subsample_positions: array<vec2f, 4> = array<vec2f, 4>(
//     vec2f(0.375, 0.125),
//     vec2f(0.875, 0.375),
//     vec2f(0.125, 0.625),
//     vec2f(0.625, 0.875)
// );
//
// Note that the algorithm should still produce _some_ edges if this is not the case!
@fragment
fn main(in: FragmentInput) -> @location(0) vec4f {
    let resolution = textureDimensions(mask_texture).xy;
    let center_coord = vec2i(vec2f(resolution) * in.texcoord);
    let max_coord = vec2i(resolution) - vec2i(1);

    //let num_samples = textureNumSamples(mask_texture);
    // TODO(andreas): Should we assert somehow on textureNumSamples here?

    let mask_top_left = textureLoad(mask_texture, center_coord, 0).xy;
    let mask_right_top = textureLoad(mask_texture, center_coord, 1).xy;
    let mask_left_bottom = textureLoad(mask_texture, center_coord, 2).xy;
    let mask_bottom_right = textureLoad(mask_texture, center_coord, 3).xy;

    var edge_pos_a_and_b = vec4f(0.0);
    var num_edges_a_and_b = vec2f(0.0);

    // Internal samples across the center point
    // Tried weighting this higher, didn't make a difference in quality since we almost always have only a single edge.
    {
        let edge = vec2f(mask_top_left != mask_bottom_right) + vec2f(mask_right_top != mask_left_bottom);
        num_edges_a_and_b += edge;
        edge_pos_a_and_b += edge.xxyy * 0.5;
    }

    // A lot of this code is repetitive, but wgsl/naga doesn't know yet how to do static indexing from unrolled loops.

    // Sample closest neighbors top/bottom/left/right
    { // right
        let edge = has_edge(max_coord, mask_right_top, center_coord + vec2i(1, 0), 2);
        let edge_pos = vec2f(1.0, 0.5);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }
    { // bottom
        let edge = has_edge(max_coord, mask_bottom_right, center_coord + vec2i(0, 1), 0);
        let edge_pos = vec2f(0.5, 1.0);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }
    { // left
        let edge = has_edge(max_coord, mask_left_bottom, center_coord + vec2i(-1, 0), 1);
        let edge_pos = vec2f(0.0, 0.5);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }
    { // top
        let edge = has_edge(max_coord, mask_top_left, center_coord + vec2i(0, -1), 3);
        let edge_pos = vec2f(0.5, 0.0);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }

    // Sample closest neighbors diagonally.
    // This is not strictly necessary, but empirically the result looks a lot better!
    { // top-right
        let edge = has_edge(max_coord, mask_right_top, center_coord + vec2i(1, -1), 2);
        let edge_pos = vec2f(1.0, 0.0);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }
    { // bottom-right
        let edge = has_edge(max_coord, mask_bottom_right, center_coord + vec2i(1, 1), 0);
        let edge_pos = vec2f(1.0, 1.0);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }
    { // bottom-left
        let edge = has_edge(max_coord, mask_left_bottom, center_coord + vec2i(-1, 1), 1);
        let edge_pos = vec2f(0.0, 1.0);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }
    { // top-left
        let edge = has_edge(max_coord, mask_top_left, center_coord + vec2i(-1, -1), 3);
        let edge_pos = vec2f(0.0, 0.0);
        //edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy; // multiplied by zero, optimize out
        num_edges_a_and_b += edge;
    }

    return compute_pixel_coords(center_coord, edge_pos_a_and_b, num_edges_a_and_b);
}
