use crate::importer_mcap::tests::util;

/// Snapshot test loading an MCAP file containing [`nav_msgs/OccupancyGrid`] messages.
#[test]
fn test_occupancy_grid() {
    let loaded_mcap = util::load_mcap(util::test_asset("ros_occupancy_grid.mcap"));

    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/map")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
