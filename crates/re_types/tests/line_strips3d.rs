use std::collections::HashMap;

use re_types::{
    archetypes::LineStrips3D,
    components::{ClassId, Color, LineStrip3D, Radius},
    Archetype as _, AsComponents as _,
};

#[test]
fn roundtrip() {
    let expected = LineStrips3D {
        #[rustfmt::skip]
        strips: vec![
            LineStrip3D::from_iter([[0., 0., 1.], [2., 1., 2.], [4., -1., 3.], [6., 0., 4.]]),
            LineStrip3D::from_iter([[0., 3., 1.], [1., 4., 2.], [2.,  2., 3.], [3., 4., 4.], [4., 2., 5.], [5., 4., 6.], [6., 3., 7.]]),
        ],
        radii: Some(vec![
            Radius(42.0), //
            Radius(43.0),
        ]),
        colors: Some(vec![
            Color::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC), //
            Color::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD),
        ]),
        labels: Some(vec![
            "hello".into(),  //
            "friend".into(), //
        ]),
        class_ids: Some(vec![
            ClassId::from(126), //
            ClassId::from(127), //
        ]),
    };

    #[rustfmt::skip]
    let strips = [
        [[0., 0., 1.], [2., 1., 2.], [4., -1., 3.], [6., 0., 4.]].to_vec(),
        [[0., 3., 1.], [1., 4., 2.], [2.,  2., 3.], [3., 4., 4.], [4., 2., 5.], [5., 4., 6.], [6., 3., 7.]].to_vec(),
    ];
    let arch = LineStrips3D::new(strips)
        .with_radii([42.0, 43.0])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_labels(["hello", "friend"])
        .with_class_ids([126, 127]);
    similar_asserts::assert_eq!(expected, arch);

    let expected_extensions: HashMap<_, _> = [
        ("points", vec!["rerun.components.LineStrip3D"]),
        ("radii", vec!["rerun.components.Radius"]),
        ("colors", vec!["rerun.components.Color"]),
        ("labels", vec!["rerun.components.Text"]),
        ("draw_order", vec!["rerun.components.DrawOrder"]),
        ("class_ids", vec!["rerun.components.ClassId"]),
        ("keypoint_ids", vec!["rerun.components.KeypointId"]),
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

    let deserialized = LineStrips3D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}

mod util;
