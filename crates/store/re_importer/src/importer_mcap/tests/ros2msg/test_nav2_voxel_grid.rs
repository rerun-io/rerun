use crate::importer_mcap::tests::util;

/// Snapshot test loading an MCAP file containing [`nav2_msgs/msg/VoxelGrid`] messages.
#[test]
fn test_nav2_voxel_grid() {
    let loaded_mcap = util::load_mcap(util::test_asset("ros_nav2_voxel_grid.mcap"));

    // Only snapshot the chunk with the payload, not the metadata chunk.
    let voxel_grid_chunk = loaded_mcap.chunks_for_entity("/voxel_grid")[1];
    insta::assert_snapshot!(format!("{:-240}", voxel_grid_chunk));
}
