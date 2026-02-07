use re_sdk_types::archetypes::Points2D;
use re_sdk_types::components::{self, ShowLabels};
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _};

#[test]
fn roundtrip() {
    let expected = Points2D {
        positions: vec![
            components::Position2D::new(1.0, 2.0), //
            components::Position2D::new(3.0, 4.0),
        ]
        .serialized(Points2D::descriptor_positions()),
        radii: vec![
            components::Radius::from(42.0), //
            components::Radius::from(43.0),
        ]
        .serialized(Points2D::descriptor_radii()),
        colors: vec![
            components::Color::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC), //
            components::Color::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD),
        ]
        .serialized(Points2D::descriptor_colors()),
        labels: vec![
            components::Text::from("hello"),  //
            components::Text::from("friend"), //
        ]
        .serialized(Points2D::descriptor_labels()),
        draw_order: components::DrawOrder::from(300.0)
            .serialized(Points2D::descriptor_draw_order()),
        class_ids: vec![
            components::ClassId::from(126), //
            components::ClassId::from(127), //
        ]
        .serialized(Points2D::descriptor_class_ids()),
        keypoint_ids: vec![
            components::KeypointId::from(2), //
            components::KeypointId::from(3), //
        ]
        .serialized(Points2D::descriptor_keypoint_ids()),
        show_labels: ShowLabels::from(false).serialized(Points2D::descriptor_show_labels()),
    };

    let arch = Points2D::new([(1.0, 2.0), (3.0, 4.0)])
        .with_radii([42.0, 43.0])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_labels(["hello", "friend"])
        .with_draw_order(300.0)
        .with_class_ids([126, 127])
        .with_keypoint_ids([2, 3])
        .with_show_labels(false);
    similar_asserts::assert_eq!(expected, arch);

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name());
    }

    let deserialized = Points2D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}
