use re_sdk_types::archetypes::AnnotationContext;
use re_sdk_types::datatypes::{ClassDescription, KeypointPair, Rgba32};
use re_sdk_types::{Archetype as _, AsComponents as _, components};

#[test]
fn roundtrip() {
    let expected = components::AnnotationContext::from([
        (1, "hello").into(),
        ClassDescription {
            info: (2, "world", Rgba32::from_rgb(3, 4, 5)).into(),
            keypoint_annotations: vec![(17, "head").into(), (42, "shoulders").into()],
            keypoint_connections: KeypointPair::vec_from([(1, 2), (3, 4)]),
        },
    ]);

    let arch = AnnotationContext::new(expected);

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow().unwrap();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name());
    }

    let deserialized = AnnotationContext::from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(arch, deserialized);
}
