//! Snapshot test loading an MCAP file containing [`foxglove.PoseInFrame`] and [`foxglove.PosesInFrame`] messages.

use crate::loader_mcap::tests::util;

#[test]
fn test_foxglove_pose_in_frame() {
    let loaded_mcap = util::load_mcap(util::test_asset("foxglove_poses_in_frame.mcap"));
    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/pose_in_frame")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}

#[test]
fn test_foxglove_poses_in_frame() {
    let loaded_mcap = util::load_mcap(util::test_asset("foxglove_poses_in_frame.mcap"));
    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/poses_in_frame")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
