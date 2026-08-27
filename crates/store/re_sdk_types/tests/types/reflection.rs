//! Tests of [`re_sdk_types::reflection`].

/// Building the reflection panics if any component's placeholder fails to serialize.
/// This test is what keeps that panic unreachable.
#[test]
fn every_placeholder_serializes() {
    let reflection = re_sdk_types::reflection::reflection();
    assert!(!reflection.components.is_empty());
    assert!(!reflection.archetypes.is_empty());
}
