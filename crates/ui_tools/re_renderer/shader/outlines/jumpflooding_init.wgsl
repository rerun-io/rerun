#import <jumpflooding_init_shared.wgsl>

@group(0) @binding(0)
var mask_texture: texture_2d<u32>;

fn has_edge(max_coord: vec2i, closest_center_sample: vec2u, sample_coord: vec2i) -> vec2f {
    // Note that `textureLoad` calls with out-of-bounds coordinates are allowed to return *any*
    // value on the texture or "transparent"/"opaque" zero.
    // See https://www.w3.org/TR/WGSL/#textureload
    // Therefore, if we want consistent behavior, we have to do the clamp ourselves.
    let clamped_coord = clamp(sample_coord, vec2i(0), max_coord);
    let mask_neighbor = textureLoad(mask_texture, clamped_coord, 0).xy;
    return vec2f(closest_center_sample != mask_neighbor);
}

// Determine *where* in texture coordinates the closest edge to the center is.
// For a more accurate version refer to `jumpflooding_init_msaa.wgsl`.
// This is a simplified version that works on WebGL.
@fragment
fn main(in: FragmentInput) -> @location(0) vec4f {
    let resolution = textureDimensions(mask_texture).xy;
    let center_coord = vec2i(vec2f(resolution) * in.texcoord);
    let max_coord = vec2i(resolution) - vec2i(1);

    let mask_center = textureLoad(mask_texture, center_coord, 0).xy;

    var edge_pos_a_and_b = vec4f(0.0);
    var num_edges_a_and_b = vec2f(0.0);

    // A lot of this code is repetitive, but wgsl/naga doesn't know yet how to do static indexing from unrolled loops.

    // Sample closest neighbors top/bottom/left/right
    { // right
        let edge = has_edge(max_coord, mask_center, center_coord + vec2i(1, 0));
        let edge_pos = vec2f(1.0, 0.5);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }
    { // bottom
        let edge = has_edge(max_coord, mask_center, center_coord + vec2i(0, 1));
        let edge_pos = vec2f(0.5, 1.0);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }
    { // left
        let edge = has_edge(max_coord, mask_center, center_coord + vec2i(-1, 0));
        let edge_pos = vec2f(0.0, 0.5);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }
    { // top
        let edge = has_edge(max_coord, mask_center, center_coord + vec2i(0, -1));
        let edge_pos = vec2f(0.5, 0.0);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }

    // Sample closest neighbors diagonally.
    { // top-right
        let edge = has_edge(max_coord, mask_center, center_coord + vec2i(1, -1));
        let edge_pos = vec2f(1.0, 0.0);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }
    { // bottom-right
        let edge = has_edge(max_coord, mask_center, center_coord + vec2i(1, 1));
        let edge_pos = vec2f(1.0, 1.0);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }
    { // bottom-left
        let edge = has_edge(max_coord, mask_center, center_coord + vec2i(-1, 1));
        let edge_pos = vec2f(0.0, 1.0);
        edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy;
        num_edges_a_and_b += edge;
    }
    { // top-left
        let edge = has_edge(max_coord, mask_center, center_coord + vec2i(-1, -1));
        let edge_pos = vec2f(0.0, 0.0);
        //edge_pos_a_and_b += vec4f(edge_pos, edge_pos) * edge.xxyy; // multiplied by zero, optimize out
        num_edges_a_and_b += edge;
    }

    return compute_pixel_coords(center_coord, edge_pos_a_and_b, num_edges_a_and_b);
}
