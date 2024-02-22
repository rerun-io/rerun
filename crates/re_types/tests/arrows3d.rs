use std::collections::HashMap;

use re_types::{
    archetypes::Arrows3D,
    components::{ClassId, Color, Position3D, Radius, Vector3D},
    datatypes::Vec3D,
    Archetype as _, AsComponents as _,
};

#[test]
fn roundtrip() {
    let expected = Arrows3D {
        vectors: vec![
            Vector3D(Vec3D([1.0, 2.0, 3.0])),
            Vector3D(Vec3D([10.0, 20.0, 30.0])),
        ],
        origins: Some(vec![
            Position3D(Vec3D([4.0, 5.0, 6.0])),    //
            Position3D(Vec3D([40.0, 50.0, 60.0])), //
        ]),
        radii: Some(vec![
            Radius(1.0), //
            Radius(10.0),
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

    let arch = Arrows3D::from_vectors([[1.0, 2.0, 3.0], [10.0, 20.0, 30.0]])
        .with_origins([[4.0, 5.0, 6.0], [40.0, 50.0, 60.0]])
        .with_radii([1.0, 10.0])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_labels(["hello", "friend"])
        .with_class_ids([126, 127]);
    similar_asserts::assert_eq!(expected, arch);

    let expected_extensions: HashMap<_, _> = [
        ("arrows", vec!["rerun.components.Arrow3D"]),
        ("radii", vec!["rerun.components.Radius"]),
        ("colors", vec!["rerun.components.Color"]),
        ("labels", vec!["rerun.components.Text"]),
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

    let deserialized = Arrows3D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}

mod util;
