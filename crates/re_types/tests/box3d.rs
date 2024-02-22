use std::collections::HashMap;

use re_types::{archetypes::Boxes3D, components, datatypes, Archetype as _, AsComponents as _};

#[test]
fn roundtrip() {
    let expected = Boxes3D {
        half_sizes: vec![
            components::HalfSizes3D::new(1.0, 2.0, 3.0), //
            components::HalfSizes3D::new(4.0, 5.0, 6.0),
        ],
        centers: Some(vec![
            components::Position3D::new(1.0, 2.0, 3.0), //
            components::Position3D::new(4.0, 5.0, 6.0),
        ]),
        rotations: Some(vec![
            components::Rotation3D::from(datatypes::Quaternion::from_xyzw([1.0, 2.0, 3.0, 4.0])),
            components::Rotation3D::from(datatypes::RotationAxisAngle::new(
                [1.0, 2.0, 3.0],
                datatypes::Angle::Radians(4.0),
            )),
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
        class_ids: Some(vec![
            components::ClassId::from(126), //
            components::ClassId::from(127), //
        ]),
    };

    let arch = Boxes3D::from_half_sizes([(1.0, 2.0, 3.0), (4.0, 5.0, 6.0)])
        .with_centers([(1.0, 2.0, 3.0), (4.0, 5.0, 6.0)])
        .with_rotations([
            components::Rotation3D::from(datatypes::Quaternion::from_xyzw([1.0, 2.0, 3.0, 4.0])),
            components::Rotation3D::from(datatypes::RotationAxisAngle::new(
                [1.0, 2.0, 3.0],
                datatypes::Angle::Radians(4.0),
            )),
        ])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_radii([42.0, 43.0])
        .with_labels(["hello", "friend"])
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

    let deserialized = Boxes3D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}

#[test]
fn from_centers_and_half_sizes() {
    let from_centers_and_half_sizes =
        Boxes3D::from_centers_and_half_sizes([(1., 2., 3.)], [(4., 6., 8.)]);
    let from_half_sizes = Boxes3D::from_half_sizes([(4., 6., 8.)]).with_centers([(1., 2., 3.)]);
    similar_asserts::assert_eq!(from_half_sizes, from_centers_and_half_sizes);
}

#[test]
fn from_sizes() {
    let from_sizes = Boxes3D::from_sizes([(4., 6., 2.)]);
    let from_half_sizes = Boxes3D::from_half_sizes([(2., 3., 1.)]);
    similar_asserts::assert_eq!(from_half_sizes, from_sizes);
}

#[test]
fn from_centers_and_sizes() {
    let from_centers_and_sizes = Boxes3D::from_centers_and_sizes([(1., 2., 3.)], [(4., 6., 8.)]);
    let from_half_sizes = Boxes3D::from_half_sizes([(2., 3., 4.)]).with_centers([(1., 2., 3.)]);
    similar_asserts::assert_eq!(from_half_sizes, from_centers_and_sizes);
}

#[test]
fn from_mins_and_sizes() {
    let from_mins_and_sizes = Boxes3D::from_mins_and_sizes([(-1., -1., -1.)], [(2., 4., 2.)]);
    let from_half_sizes = Boxes3D::from_half_sizes([(1., 2., 1.)]).with_centers([(0., 1., 0.)]);
    similar_asserts::assert_eq!(from_half_sizes, from_mins_and_sizes);
}

mod util;
