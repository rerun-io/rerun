use re_sdk_types::archetypes::Clear;
use re_sdk_types::{Archetype as _, AsComponents as _, ComponentBatch as _};

#[test]
fn roundtrip() {
    let all_expected = [
        Clear {
            is_recursive: re_sdk_types::components::ClearIsRecursive(true.into())
                .serialized(Clear::descriptor_is_recursive()),
        },
        Clear {
            is_recursive: re_sdk_types::components::ClearIsRecursive(false.into())
                .serialized(Clear::descriptor_is_recursive()),
        },
    ];

    let all_arch = [
        Clear::recursive(), //
        Clear::flat(),      //
    ];

    for (expected, arch) in all_expected.into_iter().zip(all_arch) {
        similar_asserts::assert_eq!(expected, arch);

        eprintln!("arch = {arch:#?}");
        let serialized = arch.to_arrow().unwrap();
        for (field, array) in &serialized {
            // NOTE: Keep those around please, very useful when debugging.
            // eprintln!("field = {field:#?}");
            // eprintln!("array = {array:#?}");
            eprintln!("{} = {array:#?}", field.name());
        }

        let deserialized = Clear::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(expected, deserialized);
    }
}
