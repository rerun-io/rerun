use std::collections::HashMap;

use re_types::{
    archetypes::AnnotationContext,
    components,
    datatypes::{ClassDescription, Color, KeypointPair},
    Archetype as _,
};

#[test]
fn roundtrip() {
    let expected = components::AnnotationContext::from([
        (1, "hello").into(),
        ClassDescription {
            info: (2, "world", Color::from_rgb(3, 4, 5)).into(),
            keypoint_annotations: vec![(17, "head").into(), (42, "shoulders").into()],
            keypoint_connections: KeypointPair::vec_from([(1, 2), (3, 4)]),
        },
    ]);

    let arch = AnnotationContext::new(expected);

    let expected_extensions: HashMap<_, _> = [("context", vec!["rerun.annotation_context"])].into();

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow();
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

    let deserialized = AnnotationContext::from_arrow(serialized);
    similar_asserts::assert_eq!(arch, deserialized);
}

mod util;
