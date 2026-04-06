//! Snapshot test loading an MCAP file containing [`foxglove.LocationFix`] and [`foxglove.LocationFixes`] messages.

use crate::loader_mcap::tests::util;

#[test]
fn test_foxglove_location_fix() {
    let loaded_mcap = util::load_mcap(util::test_asset("foxglove_location_fixes.mcap"));
    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/gps_fix")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}

#[test]
fn test_foxglove_location_fixes() {
    let loaded_mcap = util::load_mcap(util::test_asset("foxglove_location_fixes.mcap"));
    // Only snapshot the chunk with the payload, not the metadata chunk.
    let chunk = loaded_mcap.chunks_for_entity("/gps_fixes")[1];
    insta::assert_snapshot!(format!("{:-240}", chunk));
}
