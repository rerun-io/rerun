use crate::importer_mcap::tests::util;

/// Snapshot test loading an MCAP file containing [`std_msgs/String`] messages.
#[test]
fn test_string() {
    let loaded_mcap = util::load_mcap(util::test_asset("ros_string.mcap"));

    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/chatter")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
