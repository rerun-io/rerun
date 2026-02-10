use crate::loader_mcap::tests::util;

/// Snapshot test loading an MCAP file containing [`foxglove.CompressedVideo`] messages.
#[test]
fn test_foxglove_compressed_video() {
    let loaded_mcap = util::load_mcap(util::test_asset("foxglove_compressed_video.mcap"));

    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/compressed_video")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
