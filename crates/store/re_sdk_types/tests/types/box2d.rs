use re_sdk_types::archetypes::Boxes2D;
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _, components};

#[test]
fn roundtrip() {
    let expected = Boxes2D {
        half_sizes: vec![
            components::HalfSize2D::new(1.0, 2.0),
            components::HalfSize2D::new(3.0, 4.0),
        ]
        .serialized(Boxes2D::descriptor_half_sizes()),
        centers: vec![
            components::Position2D::new(1.0, 2.0),
            components::Position2D::new(3.0, 4.0),
        ]
        .serialized(Boxes2D::descriptor_centers()),
        colors: vec![
            components::Color::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC),
            components::Color::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD),
        ]
        .serialized(Boxes2D::descriptor_colors()),
        radii: vec![
            components::Radius::from(42.0),
            components::Radius::from(43.0),
        ]
        .serialized(Boxes2D::descriptor_radii()),
        labels: vec![
            components::Text::from("hello"),
            components::Text::from("friend"),
        ]
        .serialized(Boxes2D::descriptor_labels()),
        draw_order: components::DrawOrder(300.0.into())
            .serialized(Boxes2D::descriptor_draw_order()),
        class_ids: vec![
            components::ClassId::from(126),
            components::ClassId::from(127),
        ]
        .serialized(Boxes2D::descriptor_class_ids()),
        show_labels: components::ShowLabels(true.into())
            .serialized(Boxes2D::descriptor_show_labels()),
    };

    let arch = Boxes2D::from_half_sizes([(1.0, 2.0), (3.0, 4.0)])
        .with_centers([(1.0, 2.0), (3.0, 4.0)])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_radii([42.0, 43.0])
        .with_labels(["hello", "friend"])
        .with_draw_order(300.0)
        .with_class_ids([126, 127])
        .with_show_labels(true);
    similar_asserts::assert_eq!(expected, arch);

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name());
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
