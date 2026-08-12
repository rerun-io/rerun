//! Snapshot integration tests for importing ROS 2 messages.

use super::util::McapTestHarness;

#[test]
fn test_ros2msg_import() {
    // Note: for modularity, it's recommended to use one minimal MCAP file per message schema here.
    // This makes it easier to add/remove individual tests.
    McapTestHarness::new()
        .add("ros_camera_info.mcap", "/camera/camera_info")
        .add("ros_log.mcap", "/rosout")
        .add("ros_magnetic_field.mcap", "/imu/mag")
        .add("ros_nav2_voxel_grid.mcap", "/voxel_grid")
        .add("ros_occupancy_grid.mcap", "/map")
        .add("ros_pose_stamped.mcap", "/pose_stamped")
        .add("ros_string.mcap", "/chatter")
        .run();
}
