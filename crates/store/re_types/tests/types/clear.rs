

use re_types::{archetypes::Clear, Archetype as _, AsComponents as _};



#[test]
fn roundtrip() {
    let all_expected = [
        Clear {
            is_recursive: true.into(),
        }, //
        Clear {
            is_recursive: false.into(),
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
