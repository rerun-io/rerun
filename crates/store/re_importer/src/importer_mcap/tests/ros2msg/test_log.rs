use crate::importer_mcap::tests::util;

/// Snapshot test loading an MCAP file containing [`rcl_interfaces/Log`] messages.
#[test]
fn test_log() {
    let loaded_mcap = util::load_mcap(util::test_asset("ros_log.mcap"));

    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/rosout")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
