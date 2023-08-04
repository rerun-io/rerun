use std::collections::HashMap;

use re_types::{archetypes::TextEntry, Archetype as _};

#[test]
fn roundtrip() {
    let expected = TextEntry {
        bodies: vec!["hello", "world"].into_iter().map(Into::into).collect(),
        levels: Some(vec!["INFO", "WARN"].into_iter().map(Into::into).collect()),
    };

    let arch = TextEntry::new(["hello", "world"]).with_levels(["INFO", "WARN"]);
    similar_asserts::assert_eq!(expected, arch);

    let expected_extensions: HashMap<_, _> = [
        ("body", vec!["rerun.components.Body"]),
        ("level", vec!["rerun.components.Level"]),
    ]
    .into();

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

    let deserialized = TextEntry::from_arrow_vec(&serialized);
    similar_asserts::assert_eq!(expected, deserialized);
}

mod util;
