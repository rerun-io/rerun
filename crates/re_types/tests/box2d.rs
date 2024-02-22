use std::collections::HashMap;

use re_types::{archetypes::Boxes2D, components, Archetype as _, AsComponents as _};

#[test]
fn roundtrip() {
    let expected = Boxes2D {
        half_sizes: vec![
            components::HalfSizes2D::new(1.0, 2.0), //
            components::HalfSizes2D::new(3.0, 4.0),
        ],
        centers: Some(vec![
            components::Position2D::new(1.0, 2.0), //
            components::Position2D::new(3.0, 4.0),
        ]),
        colors: Some(vec![
            components::Color::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC), //
            components::Color::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD),
        ]),
        radii: Some(vec![
            components::Radius(42.0), //
            components::Radius(43.0),
        ]),
        labels: Some(vec![
            "hello".into(),  //
            "friend".into(), //
        ]),
        draw_order: Some(components::DrawOrder(300.0)),
        class_ids: Some(vec![
            components::ClassId::from(126), //
            components::ClassId::from(127), //
        ]),
    };

    let arch = Boxes2D::from_half_sizes([(1.0, 2.0), (3.0, 4.0)])
        .with_centers([(1.0, 2.0), (3.0, 4.0)])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_radii([42.0, 43.0])
        .with_labels(["hello", "friend"])
        .with_draw_order(300.0)
        .with_class_ids([126, 127]);
    similar_asserts::assert_eq!(expected, arch);

    let expected_extensions: HashMap<_, _> = [
        ("half_sizes", vec!["rerun.components.HalfSize2D"]),
        ("centers", vec!["rerun.components.Position2D"]),
        ("colors", vec!["rerun.components.Color"]),
        ("radii", vec!["rerun.components.Radius"]),
        ("labels", vec!["rerun.components.Label"]),
        ("draw_order", vec!["rerun.components.DrawOrder"]),
        ("class_ids", vec!["rerun.components.ClassId"]),
        ("instance_keys", vec!["rerun.components.InstanceKey"]),
    ]
    .into();

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name);

        // TODO(cmc): Re-enable extensions and these assertions once `arrow2-convert`
        // has been fully replaced.
        if false {
            util::assert_extensions(
                &**array,
                expected_extensions[field.name.as_str()].as_slice(),
            );
        }
    }

    let deserialized = Boxes2D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}

#[test]
fn from_centers_and_half_sizes() {
    let from_centers_and_half_sizes = Boxes2D::from_centers_and_half_sizes([(1., 2.)], [(4., 6.)]);
    let from_half_sizes = Boxes2D::from_half_sizes([(4., 6.)]).with_centers([(1., 2.)]);
    similar_asserts::assert_eq!(from_half_sizes, from_centers_and_half_sizes);
}

#[test]
fn from_sizes() {
    let from_sizes = Boxes2D::from_sizes([(4., 6.)]);
    let from_half_sizes = Boxes2D::from_half_sizes([(2., 3.)]);
    similar_asserts::assert_eq!(from_half_sizes, from_sizes);
}

#[test]
fn from_centers_and_sizes() {
    let from_centers_and_sizes = Boxes2D::from_centers_and_sizes([(1., 2.)], [(4., 6.)]);
    let from_half_sizes = Boxes2D::from_half_sizes([(2., 3.)]).with_centers([(1., 2.)]);
    similar_asserts::assert_eq!(from_half_sizes, from_centers_and_sizes);
}

#[test]
fn from_mins_and_sizes() {
    let from_mins_and_sizes = Boxes2D::from_mins_and_sizes([(-1., -1.)], [(2., 4.)]);
    let from_half_sizes = Boxes2D::from_half_sizes([(1., 2.)]).with_centers([(0., 1.)]);
    similar_asserts::assert_eq!(from_half_sizes, from_mins_and_sizes);
}

mod util;
