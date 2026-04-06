//! Snapshot test loading an MCAP file containing [`foxglove.CompressedImage`] messages.

use crate::loader_mcap::tests::util;

#[test]
fn test_foxglove_compressed_image() {
    let loaded_mcap = util::load_mcap(util::test_asset("foxglove_compressed_image.mcap"));
    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/camera/compressed_image")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
