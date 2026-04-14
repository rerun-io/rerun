//! Snapshot test loading an MCAP file containing [`foxglove.PointCloud`] messages.

use crate::importer_mcap::tests::util;

#[test]
fn test_foxglove_point_cloud() {
    let loaded_mcap = util::load_mcap(util::test_asset("foxglove_point_cloud.mcap"));
    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/point_cloud")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}

#[test]
fn test_foxglove_point_cloud_with_pose() {
    let loaded_mcap = util::load_mcap(util::test_asset("foxglove_point_cloud.mcap"));
    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/point_cloud_with_pose")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
