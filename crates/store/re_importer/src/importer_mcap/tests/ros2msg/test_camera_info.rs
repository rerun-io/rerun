use crate::importer_mcap::tests::util;

/// Snapshot test loading an MCAP file containing [`sensor_msgs/CameraInfo`] messages.
#[test]
fn test_camera_info() {
    let loaded_mcap = util::load_mcap(util::test_asset("ros_camera_info.mcap"));

    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/camera/camera_info")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
