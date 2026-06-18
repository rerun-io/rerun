//! Snapshot test loading an MCAP file containing [`foxglove.VoxelGrid`] messages.

use crate::importer_mcap::tests::util;

#[test]
fn test_foxglove_voxel_grid() {
    let loaded_mcap = util::load_mcap(util::test_asset("foxglove_voxel_grid.mcap"));
    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/voxel_grid")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
