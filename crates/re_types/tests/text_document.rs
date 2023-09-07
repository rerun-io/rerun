use std::collections::HashMap;

use re_types::{archetypes::TextDocument, Archetype as _};

#[test]
fn roundtrip() {
    let expected = TextDocument {
        body: "This is the contents of the text document.".into(),
    };

    let arch = TextDocument::new("This is the contents of the text document.");
    similar_asserts::assert_eq!(expected, arch);

    let expected_extensions: HashMap<_, _> = [("body", vec!["rerun.components.Text"])].into();

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

    let deserialized = TextDocument::try_from_arrow(serialized).unwrap();
    similar_asserts::assert_eq!(expected, deserialized);
}

mod util;
