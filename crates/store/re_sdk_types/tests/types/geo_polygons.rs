use re_sdk_types::archetypes::GeoPolygons;
use re_sdk_types::components::{Color, GeoPolygon};
use re_sdk_types::{Archetype as _, AsComponents as _};

#[test]
fn polygons_with_different_ring_counts_roundtrip() {
    let first = GeoPolygon::from_lat_lon([
        vec![[10.0, 20.0], [10.0, 24.0], [14.0, 24.0], [14.0, 20.0]],
        vec![[11.0, 21.0], [11.0, 22.0], [12.0, 21.0]],
    ]);
    let second = GeoPolygon::from_lat_lon([[[30.0, 40.0], [30.0, 41.0], [31.0, 40.0]]]);
    assert_eq!(&first.ring_lengths[..], &[4, 3]);
    assert_eq!(first.lat_lon.len(), 7);
    assert_eq!(&second.ring_lengths[..], &[3]);
    let original = GeoPolygons::new([first, second])
        .with_colors([Color::from_unmultiplied_rgba(10, 20, 30, 128)]);
    let restored = GeoPolygons::from_arrow(original.to_arrow().unwrap()).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn empty_polygon_batch_roundtrips_for_timeline_clearing() {
    let original = GeoPolygons::new(Vec::<GeoPolygon>::new());
    let restored = GeoPolygons::from_arrow(original.to_arrow().unwrap()).unwrap();
    assert_eq!(original, restored);
}
