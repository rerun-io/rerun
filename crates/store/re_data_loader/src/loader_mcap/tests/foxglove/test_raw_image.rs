use crate::loader_mcap::tests::util;

/// Snapshot test loading an MCAP file containing [`foxglove.RawImage`] messages.
#[test]
fn test_foxglove_raw_image() {
    let loaded_mcap = util::load_mcap(util::test_asset("foxglove_raw_image.mcap"));

    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/camera/raw_image")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
