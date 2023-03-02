#import <../types.wgsl>
#import <../screen_triangle_vertex.wgsl>

@group(0) @binding(0)
var mask_texture: texture_multisampled_2d<u32>;

// Sample positions for 4 MSAA samples are actually standardized accross all APIs we care about!
// Vulkan: https://registry.khronos.org/vulkan/specs/1.3-khr-extensions/html/chap25.html#primsrast-multisampling
// Metal: https://developer.apple.com/documentation/metal/mtldevice/2866120-getdefaultsamplepositions
// DX12 does *not* specify the sampling pattern. However DX11 does, again the same for 4 samples:
// https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ne-d3d11-d3d11_standard_multisample_quality_levels
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

    var sub_edge_pos = Vec4(0.0);
    var num_edges = Vec2(0.0);


    for (var sample_idx: i32 = 0; sample_idx < 4; sample_idx += 1) {
        let center = textureLoad(mask_texture, center_coord, sample_idx).xy;
        let up = textureLoad(mask_texture, center_coord - UVec2(0u, 1u), sample_idx).xy;
        let down = textureLoad(mask_texture, center_coord + UVec2(0u, 1u), sample_idx).xy;
        let left = textureLoad(mask_texture, center_coord - UVec2(1u, 0u), sample_idx).xy;
        let right = textureLoad(mask_texture, center_coord + UVec2(1u, 0u), sample_idx).xy;

        let has_edge_up = Vec2(center != up);
        let edge_pos_up = subsample_positions[sample_idx] + Vec2(0.0, 0.5);
        sub_edge_pos += Vec4(edge_pos_up * has_edge_up.x, edge_pos_up  * has_edge_up.y);
        num_edges += has_edge_up;

        let has_edge_down = Vec2(center != down);
        let edge_pos_down = subsample_positions[sample_idx] - Vec2(0.0, 0.5);
        sub_edge_pos += Vec4(edge_pos_down * has_edge_down.x, edge_pos_down  * has_edge_down.y);
        num_edges += has_edge_down;

        let has_edge_right = Vec2(center != right);
        let edge_pos_right = subsample_positions[sample_idx] + Vec2(0.5, 0.0);
        sub_edge_pos += Vec4(edge_pos_right * has_edge_right.x, edge_pos_right  * has_edge_right.y);
        num_edges += has_edge_right;

        let has_edge_left = Vec2(center != left);
        let edge_pos_left = subsample_positions[sample_idx] - Vec2(0.5, 0.0);
        sub_edge_pos += Vec4(edge_pos_left * has_edge_left.x, edge_pos_left  * has_edge_left.y);
        num_edges += has_edge_left;

        // num_edges += Vec2(center != down);
        // num_edges += Vec2(center != left);
        // num_edges += Vec2(center != right);
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
