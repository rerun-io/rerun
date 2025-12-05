use re_sdk_types::archetypes::Points3D;
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _, components};

#[test]
fn roundtrip() {
    let expected = Points3D {
        positions: vec![
            components::Position3D::new(1.0, 2.0, 3.0), //
            components::Position3D::new(4.0, 5.0, 6.0),
        ]
        .serialized(Points3D::descriptor_positions()),
        radii: vec![
            components::Radius::from(42.0), //
            components::Radius::from(43.0),
        ]
        .serialized(Points3D::descriptor_radii()),
        colors: vec![
            components::Color::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC), //
            components::Color::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD),
        ]
        .serialized(Points3D::descriptor_colors()),
        labels: (vec!["hello".into(), "friend".into()] as Vec<components::Text>)
            .serialized(Points3D::descriptor_labels()),
        class_ids: vec![
            components::ClassId::from(126), //
            components::ClassId::from(127), //
        ]
        .serialized(Points3D::descriptor_class_ids()),
        keypoint_ids: vec![
            components::KeypointId::from(2), //
            components::KeypointId::from(3), //
        ]
        .serialized(Points3D::descriptor_keypoint_ids()),
        show_labels: components::ShowLabels(true.into())
            .serialized(Points3D::descriptor_show_labels()),
    };

    let arch = Points3D::new([(1.0, 2.0, 3.0), (4.0, 5.0, 6.0)])
        .with_radii([42.0, 43.0])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_labels(["hello", "friend"])
        .with_class_ids([126, 127])
        .with_keypoint_ids([2, 3])
        .with_show_labels(true);
    similar_asserts::assert_eq!(expected, arch);

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        eprintln!("field = {field:#?}");
        eprintln!("array = {array:#?}");
        // eprintln!("{} = {array:#?}", field.name());
    }

    let deserialized = Points3D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}
