use std::collections::HashMap;

use re_types::{
    archetypes::Clear, components::ClearIsRecursive, Archetype as _, AsComponents as _,
};

#[test]
fn roundtrip() {
    let all_expected = [
        Clear {
            is_recursive: ClearIsRecursive(true),
        }, //
        Clear {
            is_recursive: ClearIsRecursive(false),
        },
    ];

    let all_arch = [
        Clear::recursive(), //
        Clear::flat(),      //
    ];

    let expected_extensions: HashMap<_, _> = [
        ("recursive", vec!["rerun.components.Clear"]), //
    ]
    .into();

    for (expected, arch) in all_expected.into_iter().zip(all_arch) {
        similar_asserts::assert_eq!(expected, arch);

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

        let deserialized = Clear::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}

mod util;
