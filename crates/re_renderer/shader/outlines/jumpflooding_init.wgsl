#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

@group(0) @binding(0)
var mask_texture: texture_multisampled_2d<u32>;

// Sample positions for 4 MSAA samples are actually standardized accross all APIs we care about!
// https://registry.khronos.org/vulkan/specs/1.3-khr-extensions/html/chap25.html#primsrast-multisampling
// https://developer.apple.com/documentation/metal/mtldevice/2866120-getdefaultsamplepositions
// TODO: Link DX12
var<private> subsample_positions: array<Vec2, 4> = array<Vec2, 4>(
    Vec2(0.375, 0.125),
    Vec2(0.875, 0.375),
    Vec2(0.125, 0.625),
    Vec2(0.625, 0.875)
);

@fragment
fn main(in: VertexOutput) -> @location(0) Vec4 {
    let resolution = textureDimensions(mask_texture).xy;

    // Determine *where* (in texture coordinates) the closest edge to the center is.
    // TODO: write something about the strategy we're using here.
    //let num_samples = textureNumSamples(mask_texture);
    // TODO(andreas): Should we assert somehow on textureNumSamples here?

    let center_coord = UVec2(Vec2(resolution) * in.texcoord);
    let mask_center = array<UVec2, 4>(
        textureLoad(mask_texture, center_coord, 0).xy,
        textureLoad(mask_texture, center_coord, 1).xy,
        textureLoad(mask_texture, center_coord, 2).xy,
        textureLoad(mask_texture, center_coord, 3).xy,
    );

    var sub_edge_pos = Vec4(0.0);
    var num_edges = Vec2(0.0);

    // Internal samples in a circle.
    {
        let edge_between_0_1 = Vec2(mask_center[0] != mask_center[1]);
        let edge_pos_0_1 = (subsample_positions[0] + subsample_positions[1]) * 0.5;
        sub_edge_pos += Vec4(edge_pos_0_1 * edge_between_0_1.x, edge_pos_0_1 * edge_between_0_1.y);
        num_edges +=edge_between_0_1;

        let edge_between_1_3 = Vec2(mask_center[1] != mask_center[3]);
        let edge_pos_1_3 = (subsample_positions[1] + subsample_positions[3]) * 0.5;
        sub_edge_pos += Vec4(edge_pos_1_3 * edge_between_1_3.x, edge_pos_1_3 * edge_between_1_3.y);
        num_edges +=edge_between_1_3;

        let edge_between_3_2 = Vec2(mask_center[3] != mask_center[2]);
        let edge_pos_3_2 = (subsample_positions[3] + subsample_positions[2]) * 0.5;
        sub_edge_pos += Vec4(edge_pos_3_2 * edge_between_3_2.x, edge_pos_3_2 * edge_between_3_2.y);
        num_edges +=edge_between_3_2;

        let edge_between_2_0 = Vec2(mask_center[2] != mask_center[0]);
        let edge_pos_2_0 = (subsample_positions[2] + subsample_positions[0]) * 0.5;
        sub_edge_pos += Vec4(edge_pos_2_0 * edge_between_2_0.x, edge_pos_2_0 * edge_between_2_0.y);
        num_edges += edge_between_2_0;
    }

    // Two samples to the neighbors - closest right and closest down.
    {
        let mask_closest_right = textureLoad(mask_texture, center_coord + UVec2(1u, 0u), 2).xy;
        let mask_closest_down = textureLoad(mask_texture, center_coord + UVec2(0u, 1u), 0).xy;

        let edge_between_1_right = Vec2(mask_center[1] != mask_closest_right);
        let edge_pos_1_right = Vec2(1.0, 0.5);
        sub_edge_pos += Vec4(edge_pos_1_right * edge_between_1_right.x, edge_pos_1_right * edge_between_1_right.y);
        num_edges +=edge_between_1_right;

        let edge_between_3_down = Vec2(mask_center[3] != mask_closest_down);
        let edge_pos_3_down = Vec2(0.5, 1.0);
        sub_edge_pos += Vec4(edge_pos_3_down * edge_between_3_down.x, edge_pos_3_down * edge_between_3_down.y);
        num_edges += edge_between_3_down;
    }

    sub_edge_pos = Vec4(sub_edge_pos.xy / num_edges.x, sub_edge_pos.zw / num_edges.y);
    sub_edge_pos = sub_edge_pos * 0.5 - Vec4(0.5);

    var texcoord_channel_0: Vec2;
    if num_edges.x == 0.0 {
        texcoord_channel_0 = Vec2(-10.0);
    } else {
        texcoord_channel_0 = in.texcoord + sub_edge_pos.xy / Vec2(resolution);
    }
    var texcoord_channel_1: Vec2;
    if num_edges.y == 0.0 {
        texcoord_channel_1 = Vec2(-10.0);
    } else {
        texcoord_channel_1 = in.texcoord + sub_edge_pos.zw / Vec2(resolution);
    }

    return Vec4(texcoord_channel_0, texcoord_channel_1);
}
