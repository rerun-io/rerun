use crate::importer_mcap::tests::util;

/// Snapshot test loading an MCAP file containing [`sensor_msgs/MagneticField`] messages.
#[test]
fn test_magnetic_field() {
    let loaded_mcap = util::load_mcap(util::test_asset("ros_magnetic_field.mcap"));

    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/imu/mag")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
