#![allow(clippy::redundant_clone)]

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
        single_float_optional: None,
        single_string_required: "b".into(),
        single_string_optional: None,
        many_floats_optional: Some(vec![1.0, 10.0]),
        many_strings_required: vec!["1".into(), "10".into()],
        many_strings_optional: None,
        // TODO(cmc): this one is bugged.
        // many_transparent_optionals: vec![],
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
        single_float_optional: Some(1.0),
        single_string_required: "a".into(),
        single_string_optional: Some("a".into()),
        many_floats_optional: Some(vec![2.0, 20.0, 200.0]),
        many_strings_required: vec!["20".into(), "200".into(), "2000".into()],
        many_strings_optional: Some(vec![
            "30".into(),
            "300".into(),
            "3000".into(),
            "30000".into(),
        ]),
        // TODO(cmc): this one is bugged.
        // many_transparent_optionals: vec![],
    };

    let arch = AffixFuzzer1::new(
        fuzzy1.clone(),
        fuzzy2.clone(),
        fuzzy3.clone(),
        fuzzy4.clone(),
        fuzzy5.clone(),
        fuzzy6.clone(),
        fuzzy7_1.clone(),
        [fuzzy1.clone(), fuzzy1.clone(), fuzzy1.clone()],
        [fuzzy2.clone(), fuzzy2.clone(), fuzzy2.clone()],
        [fuzzy3.clone(), fuzzy3.clone(), fuzzy3.clone()],
        [fuzzy4.clone(), fuzzy4.clone(), fuzzy4.clone()],
        [fuzzy5.clone(), fuzzy5.clone(), fuzzy5.clone()],
        [fuzzy6.clone(), fuzzy6.clone(), fuzzy6.clone()],
        [fuzzy7_1.clone(), fuzzy7_2.clone()],
    )
    .with_fuzz2001(fuzzy1.clone())
    .with_fuzz2003(fuzzy3.clone())
    .with_fuzz2005(fuzzy5.clone())
    .with_fuzz2007(fuzzy7_2.clone())
    .with_fuzz2102([fuzzy2.clone(), fuzzy2.clone(), fuzzy2.clone()])
    .with_fuzz2104([fuzzy4.clone(), fuzzy4.clone(), fuzzy4.clone()])
    .with_fuzz2106([fuzzy6.clone(), fuzzy6.clone(), fuzzy6.clone()]);

    eprintln!("arch = {arch:#?}");
    let serialized = arch.to_arrow();
    for (field, array) in &serialized {
        // NOTE: Keep those around please, very useful when debugging.
        // eprintln!("field = {field:#?}");
        // eprintln!("array = {array:#?}");
        eprintln!("{} = {array:#?}", field.name);
    }

    // TODO(cmc): deserialize
}
