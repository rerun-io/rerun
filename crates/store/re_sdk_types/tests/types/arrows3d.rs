use re_sdk_types::archetypes::Arrows3D;
use re_sdk_types::components::{ClassId, Color, Position3D, Radius, ShowLabels, Text, Vector3D};
use re_sdk_types::datatypes::Vec3D;
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _};

#[test]
fn roundtrip() {
    let expected = Arrows3D {
        vectors: vec![
            Vector3D(Vec3D([1.0, 2.0, 3.0])),
            Vector3D(Vec3D([10.0, 20.0, 30.0])),
        ]
        .serialized(Arrows3D::descriptor_vectors()),
        origins: vec![
            Position3D(Vec3D([4.0, 5.0, 6.0])),    //
            Position3D(Vec3D([40.0, 50.0, 60.0])), //
        ]
        .serialized(Arrows3D::descriptor_origins()),
        radii: vec![
            Radius::from(1.0), //
            Radius::from(10.0),
        ]
        .serialized(Arrows3D::descriptor_radii()),
        colors: vec![
            Color::from_unmultiplied_rgba(0xAA, 0x00, 0x00, 0xCC), //
            Color::from_unmultiplied_rgba(0x00, 0xBB, 0x00, 0xDD),
        ]
        .serialized(Arrows3D::descriptor_colors()),
        labels: vec![
            Text::from("hello"),  //
            Text::from("friend"), //
        ]
        .serialized(Arrows3D::descriptor_labels()),
        class_ids: vec![
            ClassId::from(126), //
            ClassId::from(127), //
        ]
        .serialized(Arrows3D::descriptor_class_ids()),
        show_labels: ShowLabels(true.into()).serialized(Arrows3D::descriptor_show_labels()),
    };

    let arch = Arrows3D::from_vectors([[1.0, 2.0, 3.0], [10.0, 20.0, 30.0]])
        .with_origins([[4.0, 5.0, 6.0], [40.0, 50.0, 60.0]])
        .with_radii([1.0, 10.0])
        .with_colors([0xAA0000CC, 0x00BB00DD])
        .with_labels(["hello", "friend"])
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

    let deserialized = Arrows3D::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}
