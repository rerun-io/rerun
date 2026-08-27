//! Snapshot tests for importing Foxglove messages.

use super::util::McapTestHarness;

#[test]
fn test_foxglove_import() {
    // Note: for modularity, it's recommended to use one minimal MCAP file per message schema here.
    // This makes it easier to add/remove individual tests.
    McapTestHarness::new()
        .add("foxglove_camera_calibration.mcap", "/camera/calibration")
        .add("foxglove_compressed_image.mcap", "/camera/compressed_image")
        .add("foxglove_compressed_video.mcap", "/compressed_video")
        .add("foxglove_frame_transforms.mcap", "/frame_transform")
        .add("foxglove_frame_transforms.mcap", "/frame_transforms")
        .add("foxglove_location_fixes.mcap", "/gps_fix")
        .add("foxglove_location_fixes.mcap", "/gps_fixes")
        .add("foxglove_log.mcap", "/text_log")
        .add("foxglove_point_cloud.mcap", "/point_cloud")
        .add("foxglove_point_cloud.mcap", "/point_cloud_with_pose")
        .add("foxglove_poses_in_frame.mcap", "/pose_in_frame")
        .add("foxglove_poses_in_frame.mcap", "/poses_in_frame")
        .add("foxglove_raw_image.mcap", "/camera/raw_image")
        .add("foxglove_voxel_grid.mcap", "/voxel_grid")
        .run();
}
