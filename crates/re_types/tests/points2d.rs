use std::collections::HashMap;

use re_types::{archetypes::Points2D, components, Archetype as _};

#[test]
fn roundtrip() {
    let expected = Points2D {
        points: vec![
            components::Point2D::new(1.0, 2.0), //
            components::Point2D::new(3.0, 4.0),
        ],
        radii: Some(vec![
            components::Radius(42.0), //
            components::Radius(43.0),
        ]),
        colors: Some(vec![
            components::Color::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC), //
            components::Color::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD),
        ]),
        labels: Some(vec![
            components::Label("hello".to_owned()),  //
            components::Label("friend".to_owned()), //
        ]),
        draw_order: Some(components::DrawOrder(300.0)),
        class_ids: Some(vec![
            components::ClassId(126), //
            components::ClassId(127), //
        ]),
        keypoint_ids: Some(vec![
            components::KeypointId(2), //
            components::KeypointId(3), //
        ]),
        instance_keys: Some(vec![
            components::InstanceKey(u64::MAX - 1), //
            components::InstanceKey(u64::MAX),
        ]),
    };

    let arch = Points2D::new([(1.0, 2.0), (3.0, 4.0)])
        .with_radii([42.0, 43.0])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_labels(["hello", "friend"])
        .with_draw_order(300.0)
        .with_class_ids([126, 127])
        .with_keypoint_ids([2, 3])
        .with_instance_keys([u64::MAX - 1, u64::MAX]);
    similar_asserts::assert_eq!(expected, arch);

    let expected_extensions: HashMap<_, _> = [
        ("points", vec!["rerun.components.Point2D"]),
        ("radii", vec!["rerun.components.Radius"]),
        ("colors", vec!["rerun.components.Color"]),
        ("labels", vec!["rerun.components.Label"]),
        ("draw_order", vec!["rerun.components.DrawOrder"]),
        ("class_ids", vec!["rerun.components.ClassId"]),
        ("keypoint_ids", vec!["rerun.components.KeypointId"]),
        ("instance_keys", vec!["rerun.components.InstanceKey"]),
    ]
    .into();

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name);
        util::assert_extensions(
            &**array,
            expected_extensions[field.name.as_str()].as_slice(),
        );
    }

    let deserialized = Points2D::from_arrow(serialized);
    similar_asserts::assert_eq!(expected, deserialized);
}

mod util;
