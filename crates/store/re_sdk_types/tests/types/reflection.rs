//! Tests of [`re_sdk_types::reflection`].

/// Building the reflection panics if any component's placeholder fails to serialize.
/// This test is what keeps that panic unreachable.
#[test]
fn every_placeholder_serializes() {
    let reflection = re_sdk_types::reflection::reflection();
    assert!(!reflection.components.is_empty());
    assert!(!reflection.archetypes.is_empty());
}

#[test]
fn view_applicability_is_queryable_in_both_directions() {
    use re_sdk_types::{Archetype as _, View as _};

    let reflection = re_sdk_types::reflection::reflection();
    let points3d = re_sdk_types::archetypes::Points3D::name();
    let spatial3d = re_sdk_types::blueprint::views::Spatial3DView::identifier();
    let map = re_sdk_types::blueprint::views::MapView::identifier();

    assert!(reflection.views[&spatial3d].supports_archetype(points3d));
    assert!(!reflection.views[&map].supports_archetype(points3d));

    let views = reflection
        .views_for_archetype(points3d)
        .collect::<nohash_hasher::IntSet<_>>();
    assert!(views.contains(&spatial3d));
    assert!(!views.contains(&map));
}
