//! Snapshot test loading an MCAP file containing [`foxglove.CameraCalibration`] messages.

use crate::importer_mcap::tests::util;

#[test]
fn test_foxglove_camera_calibration() {
    let loaded_mcap = util::load_mcap(util::test_asset("foxglove_camera_calibration.mcap"));
    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/camera/calibration")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
