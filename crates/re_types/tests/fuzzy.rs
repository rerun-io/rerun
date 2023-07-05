#![allow(clippy::redundant_clone)]

use std::collections::HashMap;

use re_types::{archetypes::AffixFuzzer1, Archetype as _};

#[test]
fn roundtrip() {
    let fuzzy1 = re_types::components::AffixFuzzer1 {
        single_required: re_types::datatypes::AffixFuzzer1 {
            single_float_optional: Some(1.0),
            single_string_required: "a".into(),
            single_string_optional: Some("a".into()),
            many_floats_optional: Some(vec![1.0, 10.0, 100.0]),
            many_strings_required: vec!["1".into(), "2".into()],
            many_strings_optional: Some(vec!["10".into(), "20".into()]),
        },
    };

    let fuzzy2 = re_types::components::AffixFuzzer2(re_types::datatypes::AffixFuzzer1 {
        single_float_optional: None,
        single_string_required: "b".into(),
        single_string_optional: None,
        many_floats_optional: Some(vec![2.0, 20.0, 200.0]),
        many_strings_required: vec!["3".into(), "4".into()],
        many_strings_optional: None,
    });

    let fuzzy3 = re_types::components::AffixFuzzer3 {
        single_required: re_types::datatypes::AffixFuzzer1 {
            single_float_optional: Some(3.0),
            single_string_required: "c".into(),
            single_string_optional: Some("c".into()),
            many_floats_optional: Some(vec![3.0, 30.0, 300.0]),
            many_strings_required: vec!["5".into(), "6".into()],
            many_strings_optional: Some(vec!["50".into(), "60".into()]),
        },
    };

    let fuzzy4 = re_types::components::AffixFuzzer4 {
        single_optional: Some(re_types::datatypes::AffixFuzzer1 {
            single_float_optional: None,
            single_string_required: "d".into(),
            single_string_optional: None,
            many_floats_optional: Some(vec![4.0, 40.0, 400.0]),
            many_strings_required: vec!["7".into(), "8".into()],
            many_strings_optional: None,
        }),
    };

    let fuzzy5 = re_types::components::AffixFuzzer5(None);
    // let fuzzy5 = re_types::components::AffixFuzzer5(Some(re_types::datatypes::AffixFuzzer1 {
    //     single_float_optional: None,
    //     single_string_required: "d".into(),
    //     single_string_optional: None,
    //     many_floats_optional: Some(vec![4.0, 40.0, 400.0]),
    //     many_strings_required: vec!["7".into(), "8".into()],
    //     many_strings_optional: None,
    // }));

    let fuzzy6 = re_types::components::AffixFuzzer6 {
        single_optional: None,
    };
    // let fuzzy6 = re_types::components::AffixFuzzer6 {
    //     single_optional: Some(re_types::datatypes::AffixFuzzer1 {
    //         // single_float_optional: None,
    //         single_string_required: "d".into(),
    //         // single_string_optional: None,
    //         //             many_floats_optional: Some(vec![4.0, 40.0, 400.0]),
    //         //             many_strings_required: vec!["7".into(), "8".into()],
    //         //             many_strings_optional: None,
    //     }),
    // };

    let fuzzy7_1 = re_types::components::AffixFuzzer7 {
        many_optional: None,
    };
    let fuzzy7_2 = re_types::components::AffixFuzzer7 {
        many_optional: Some(vec![re_types::datatypes::AffixFuzzer1 {
            single_float_optional: None,
            single_string_required: "d".into(),
            single_string_optional: None,
            many_floats_optional: Some(vec![4.0, 40.0, 400.0]),
            many_strings_required: vec!["7".into(), "8".into()],
            many_strings_optional: None,
        }]),
    };

    let fuzzy8_1 = re_types::components::AffixFuzzer8 {
        single_float_optional: None,
    };
    let fuzzy8_2 = re_types::components::AffixFuzzer8 {
        single_float_optional: Some(1.0),
    };

    let fuzzy9_1 = re_types::components::AffixFuzzer9 {
        single_string_required: "b".into(),
    };
    let fuzzy9_2 = re_types::components::AffixFuzzer9 {
        single_string_required: "a".into(),
    };

    let fuzzy10_1 = re_types::components::AffixFuzzer10 {
        single_string_optional: None,
    };
    let fuzzy10_2 = re_types::components::AffixFuzzer10 {
        single_string_optional: Some("a".into()),
    };

    let fuzzy11_1 = re_types::components::AffixFuzzer11 {
        many_floats_optional: Some(vec![1.0, 10.0]),
    };
    let fuzzy11_2 = re_types::components::AffixFuzzer11 {
        many_floats_optional: Some(vec![2.0, 20.0, 200.0]),
    };

    let fuzzy12_1 = re_types::components::AffixFuzzer12 {
        many_strings_required: vec!["1".into(), "10".into()],
    };
    let fuzzy12_2 = re_types::components::AffixFuzzer12 {
        many_strings_required: vec!["20".into(), "200".into(), "2000".into()],
    };

    let fuzzy13_1 = re_types::components::AffixFuzzer13 {
        many_strings_optional: None,
    };
    let fuzzy13_2 = re_types::components::AffixFuzzer13 {
        many_strings_optional: Some(vec![
            "30".into(),
            "300".into(),
            "3000".into(),
            "30000".into(),
        ]),
    };

    let arch = AffixFuzzer1::new(
        fuzzy1.clone(),
        fuzzy2.clone(),
        fuzzy3.clone(),
        fuzzy4.clone(),
        fuzzy5.clone(),
        fuzzy6.clone(),
        fuzzy7_1.clone(),
        fuzzy8_1.clone(),
        fuzzy9_1.clone(),
        fuzzy10_1.clone(),
        fuzzy11_1.clone(),
        fuzzy12_1.clone(),
        fuzzy13_1.clone(),
        [fuzzy1.clone(), fuzzy1.clone(), fuzzy1.clone()],
        [fuzzy2.clone(), fuzzy2.clone(), fuzzy2.clone()],
        [fuzzy3.clone(), fuzzy3.clone(), fuzzy3.clone()],
        [fuzzy4.clone(), fuzzy4.clone(), fuzzy4.clone()],
        [fuzzy5.clone(), fuzzy5.clone(), fuzzy5.clone()],
        [fuzzy6.clone(), fuzzy6.clone(), fuzzy6.clone()],
        [fuzzy7_1.clone(), fuzzy7_2.clone(), fuzzy7_1.clone()],
        [fuzzy8_1.clone(), fuzzy8_2.clone(), fuzzy8_1.clone()],
        [fuzzy9_1.clone(), fuzzy9_2.clone(), fuzzy9_1.clone()],
        [fuzzy10_1.clone(), fuzzy10_2.clone(), fuzzy10_1.clone()],
        [fuzzy11_1.clone(), fuzzy11_2.clone(), fuzzy11_1.clone()],
        [fuzzy12_1.clone(), fuzzy12_2.clone(), fuzzy12_1.clone()],
        [fuzzy13_1.clone(), fuzzy13_2.clone(), fuzzy13_1.clone()],
    )
    .with_fuzz2001(fuzzy1.clone())
    .with_fuzz2003(fuzzy3.clone())
    .with_fuzz2005(fuzzy5.clone())
    .with_fuzz2102([fuzzy2.clone(), fuzzy2.clone(), fuzzy2.clone()])
    .with_fuzz2104([fuzzy4.clone(), fuzzy4.clone(), fuzzy4.clone()])
    .with_fuzz2106([fuzzy6.clone(), fuzzy6.clone(), fuzzy6.clone()])
    .with_fuzz2108([fuzzy8_1.clone(), fuzzy8_2.clone(), fuzzy8_1.clone()])
    .with_fuzz2110([fuzzy10_1.clone(), fuzzy10_2.clone(), fuzzy10_1.clone()])
    .with_fuzz2112([fuzzy12_1.clone(), fuzzy12_2.clone(), fuzzy12_1.clone()]);

    #[rustfmt::skip]
    let expected_extensions: HashMap<_, _> = [
        ("fuzz1001", vec!["rerun.testing.components.AffixFuzzer1", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1002", vec!["rerun.testing.components.AffixFuzzer2", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1003", vec!["rerun.testing.components.AffixFuzzer3", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1004", vec!["rerun.testing.components.AffixFuzzer4", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1005", vec!["rerun.testing.components.AffixFuzzer5", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1006", vec!["rerun.testing.components.AffixFuzzer6", "rerun.testing.datatypes.AffixFuzzer1"]),

        ("fuzz1101", vec!["rerun.testing.components.AffixFuzzer1", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1102", vec!["rerun.testing.components.AffixFuzzer2", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1103", vec!["rerun.testing.components.AffixFuzzer3", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1104", vec!["rerun.testing.components.AffixFuzzer4", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1105", vec!["rerun.testing.components.AffixFuzzer5", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1106", vec!["rerun.testing.components.AffixFuzzer6", "rerun.testing.datatypes.AffixFuzzer1"]),

        ("fuzz2001", vec!["rerun.testing.components.AffixFuzzer1", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2002", vec!["rerun.testing.components.AffixFuzzer2", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2003", vec!["rerun.testing.components.AffixFuzzer3", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2004", vec!["rerun.testing.components.AffixFuzzer4", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2005", vec!["rerun.testing.components.AffixFuzzer5", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2006", vec!["rerun.testing.components.AffixFuzzer6", "rerun.testing.datatypes.AffixFuzzer1"]),

        ("fuzz2101", vec!["rerun.testing.components.AffixFuzzer1", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2102", vec!["rerun.testing.components.AffixFuzzer2", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2103", vec!["rerun.testing.components.AffixFuzzer3", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2104", vec!["rerun.testing.components.AffixFuzzer4", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2105", vec!["rerun.testing.components.AffixFuzzer5", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2106", vec!["rerun.testing.components.AffixFuzzer6", "rerun.testing.datatypes.AffixFuzzer1"]),
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

    let deserialized = AffixFuzzer1::from_arrow(serialized);
    similar_asserts::assert_eq!(arch, deserialized);
}

mod util;
