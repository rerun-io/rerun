use re_sdk_types::archetypes::Boxes3D;
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _, components, datatypes};

#[test]
fn roundtrip() {
    let expected = Boxes3D {
        half_sizes: vec![
            components::HalfSize3D::new(1.0, 2.0, 3.0), //
            components::HalfSize3D::new(4.0, 5.0, 6.0),
        ]
        .serialized(Boxes3D::descriptor_half_sizes()),
        centers: vec![
            components::Translation3D::new(1.0, 2.0, 3.0), //
            components::Translation3D::new(4.0, 5.0, 6.0),
        ]
        .serialized(Boxes3D::descriptor_centers()),
        quaternions: vec![components::RotationQuat::from(
            datatypes::Quaternion::from_xyzw([1.0, 2.0, 3.0, 4.0]),
        )]
        .serialized(Boxes3D::descriptor_quaternions()),
        rotation_axis_angles: vec![components::RotationAxisAngle::new(
            [1.0, 2.0, 3.0],
            datatypes::Angle::from_radians(4.0),
        )]
        .serialized(Boxes3D::descriptor_rotation_axis_angles()),
        colors: vec![
            components::Color::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC),
            components::Color::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD),
        ]
        .serialized(Boxes3D::descriptor_colors()),
        radii: vec![
            components::Radius::from(42.0),
            components::Radius::from(43.0),
        ]
        .serialized(Boxes3D::descriptor_radii()),
        fill_mode: components::FillMode::Solid.serialized(Boxes3D::descriptor_fill_mode()),
        labels: vec![
            components::Text::from("hello"),
            components::Text::from("friend"),
        ]
        .serialized(Boxes3D::descriptor_labels()),
        class_ids: vec![
            components::ClassId::from(126),
            components::ClassId::from(127),
        ]
        .serialized(Boxes3D::descriptor_class_ids()),
        show_labels: components::ShowLabels(false.into())
            .serialized(Boxes3D::descriptor_show_labels()),
    };

    let arch = Boxes3D::from_half_sizes([(1.0, 2.0, 3.0), (4.0, 5.0, 6.0)])
        .with_centers([(1.0, 2.0, 3.0), (4.0, 5.0, 6.0)])
        .with_quaternions([datatypes::Quaternion::from_xyzw([1.0, 2.0, 3.0, 4.0])])
        .with_rotation_axis_angles([datatypes::RotationAxisAngle::new(
            [1.0, 2.0, 3.0],
            datatypes::Angle::from_radians(4.0),
        )])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_radii([42.0, 43.0])
        .with_fill_mode(components::FillMode::Solid)
        .with_labels(["hello", "friend"])
        .with_class_ids([126, 127])
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
