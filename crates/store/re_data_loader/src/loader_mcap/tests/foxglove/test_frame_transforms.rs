//! Snapshot test loading an MCAP file containing [`foxglove.FrameTransform`] and [`foxglove.FrameTransforms`] messages.

use crate::loader_mcap::tests::util;

#[test]
fn test_foxglove_frame_transform() {
    let loaded_mcap = util::load_mcap(util::test_asset("foxglove_frame_transforms.mcap"));
    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/frame_transform")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}

#[test]
fn test_foxglove_frame_transforms() {
    let loaded_mcap = util::load_mcap(util::test_asset("foxglove_frame_transforms.mcap"));
    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/frame_transforms")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
