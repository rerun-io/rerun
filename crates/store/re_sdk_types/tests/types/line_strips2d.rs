use re_sdk_types::archetypes::LineStrips2D;
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _, components};

#[test]
fn roundtrip() {
    let expected = LineStrips2D {
        #[rustfmt::skip]
        strips: vec![
            components::LineStrip2D::from_iter([[0., 0.], [2., 1.], [4., -1.], [6., 0.]]), //
            components::LineStrip2D::from_iter([[0., 3.], [1., 4.], [2., 2.], [3., 4.], [4., 2.], [5., 4.], [6., 3.]]), //
        ]
        .serialized(LineStrips2D::descriptor_strips()),
        radii: vec![
            components::Radius::from(42.0), //
            components::Radius::from(43.0),
        ]
        .serialized(LineStrips2D::descriptor_radii()),
        colors: vec![
            components::Color::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC), //
            components::Color::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD),
        ]
        .serialized(LineStrips2D::descriptor_colors()),
        labels: (vec!["hello".into(), "friend".into()] as Vec<components::Text>)
            .serialized(LineStrips2D::descriptor_labels()),
        draw_order: vec![components::DrawOrder(300.0.into())]
            .serialized(LineStrips2D::descriptor_draw_order()),
        class_ids: vec![
            components::ClassId::from(126), //
            components::ClassId::from(127), //
        ]
        .serialized(LineStrips2D::descriptor_class_ids()),
        show_labels: components::ShowLabels(false.into())
            .serialized(LineStrips2D::descriptor_show_labels()),
    };

    #[rustfmt::skip]
    let strips = [
        [[0., 0.], [2., 1.], [4., -1.], [6., 0.]].to_vec(),
        [[0., 3.], [1., 4.], [2., 2.], [3., 4.], [4., 2.], [5., 4.], [6., 3.]].to_vec(),
    ];
    let arch = LineStrips2D::new(strips)
        .with_radii([42.0, 43.0])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_labels(["hello", "friend"])
        .with_draw_order(300.0)
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

    let deserialized = LineStrips2D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}
