use crate::importer_mcap::tests::util;

/// Snapshot test loading an MCAP file containing [`geometry_msgs/PoseStamped`] messages.
#[test]
fn test_pose_stamped() {
    let loaded_mcap = util::load_mcap(util::test_asset("ros_pose_stamped.mcap"));

    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/pose_stamped")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
