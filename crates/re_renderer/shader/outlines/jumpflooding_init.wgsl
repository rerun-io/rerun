#import <jumpflooding_init_shared.wgsl>

@group(0) @binding(0)
var mask_texture: texture_2d<u32>;

fn has_edge(closest_center_sample: UVec2, sample_coord: IVec2) -> Vec2 {
    let mask_neighbor = textureLoad(mask_texture, sample_coord, 0).xy;
    return Vec2(closest_center_sample != mask_neighbor);
}

// Determine *where* in texture coordinates the closest edge to the center is.
// For a more accurate version refer to `jumpflooding_init_msaa.wgsl`.
// This is a simplified version that works on WebGL.
@fragment
fn main(in: FragmentInput) -> @location(0) Vec4 {
    let resolution = textureDimensions(mask_texture).xy;
    let center_coord = IVec2(Vec2(resolution) * in.texcoord);

    let mask_center = textureLoad(mask_texture, center_coord, 0).xy;

    var edge_pos_a_and_b = Vec4(0.0);
    var num_edges_and_b = Vec2(0.0);

    // A lot of this code is repetetive, but wgsl/naga doesn't know yet how to do static indexing from unrolled loops.

    // Sample closest neighbors top/bottom/left/right
    { // right
        let has_edge = has_edge(mask_center, center_coord + IVec2(1, 0));
        let edge_pos = Vec2(1.0, 0.5);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }
    { // bottom
        let has_edge = has_edge(mask_center, center_coord + IVec2(0, 1));
        let edge_pos = Vec2(0.5, 1.0);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }
    { // left
        let has_edge = has_edge(mask_center, center_coord + IVec2(-1, 0));
        let edge_pos = Vec2(0.0, 0.5);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }
    { // top
        let has_edge = has_edge(mask_center, center_coord + IVec2(0, -1));
        let edge_pos = Vec2(0.5, 0.0);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }

    // Sample closest neighbors diagonally.
    { // top-right
        let has_edge = has_edge(mask_center, center_coord + IVec2(1, -1));
        let edge_pos = Vec2(1.0, 0.0);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }
    { // bottom-right
        let has_edge = has_edge(mask_center, center_coord + IVec2(1, 1));
        let edge_pos = Vec2(1.0, 1.0);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }
    { // bottom-left
        let has_edge = has_edge(mask_center, center_coord + IVec2(-1, 1));
        let edge_pos = Vec2(0.0, 1.0);
        edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }
    { // top-left
        let has_edge = has_edge(mask_center, center_coord + IVec2(-1, -1));
        //let edge_pos = Vec2(0.0, 0.0);
        //edge_pos_a_and_b += Vec4(edge_pos, edge_pos) * has_edge.xxyy;
        num_edges_and_b += has_edge;
    }

    return compute_pixel_coords(center_coord, edge_pos_a_and_b, num_edges_and_b);
}
