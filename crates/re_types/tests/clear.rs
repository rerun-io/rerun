use std::collections::HashMap;

use re_types::{archetypes::Clear, components::ClearIsRecursive, Archetype as _};

#[test]
fn roundtrip() {
    let all_expected = [
        Clear {
            settings: ClearIsRecursive(true),
        }, //
        Clear {
            settings: ClearIsRecursive(false),
        },
    ];

    let all_arch = [
        Clear::recursive(), //
        Clear::flat(),      //
    ];

    let expected_extensions: HashMap<_, _> = [
        ("settings", vec!["rerun.components.Clear"]), //
    ]
    .into();

    for (expected, arch) in all_expected.into_iter().zip(all_arch) {
        similar_asserts::assert_eq!(expected, arch);

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

        let deserialized = Clear::try_from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}

mod util;
