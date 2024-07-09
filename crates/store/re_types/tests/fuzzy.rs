#![allow(clippy::redundant_clone)]

use std::collections::HashMap;

use arrow2::types::f16;
use re_types::{
    testing::{
        archetypes::{AffixFuzzer1, AffixFuzzer2, AffixFuzzer3, AffixFuzzer4},
        components, datatypes,
    },
    Archetype as _, AsComponents as _,
};

#[test]
fn roundtrip() {
    let fuzzy1 = components::AffixFuzzer1(datatypes::AffixFuzzer1 {
        single_float_optional: Some(1.0),
        single_string_required: "a".into(),
        single_string_optional: Some("a".into()),
        many_floats_optional: Some(vec![1.0, 10.0, 100.0].into()),
        many_strings_required: vec!["1".into(), "2".into()],
        many_strings_optional: Some(vec!["10".into(), "20".into()]),
        flattened_scalar: 42.0,
        almost_flattened_scalar: datatypes::FlattenedScalar { value: 42.0 },
        from_parent: Some(true),
    });

    let fuzzy2 = components::AffixFuzzer2(datatypes::AffixFuzzer1 {
        single_float_optional: None,
        single_string_required: "b".into(),
        single_string_optional: None,
        many_floats_optional: Some(vec![2.0, 20.0, 200.0].into()),
        many_strings_required: vec!["3".into(), "4".into()],
        many_strings_optional: None,
        flattened_scalar: 43.0,
        almost_flattened_scalar: datatypes::FlattenedScalar { value: 43.0 },
        from_parent: Some(false),
    });

    let fuzzy3 = components::AffixFuzzer3(datatypes::AffixFuzzer1 {
        single_float_optional: Some(3.0),
        single_string_required: "c".into(),
        single_string_optional: Some("c".into()),
        many_floats_optional: Some(vec![3.0, 30.0, 300.0].into()),
        many_strings_required: vec!["5".into(), "6".into()],
        many_strings_optional: Some(vec!["50".into(), "60".into()]),
        flattened_scalar: 44.0,
        almost_flattened_scalar: datatypes::FlattenedScalar { value: 44.0 },
        from_parent: None,
    });

    let fuzzy4 = components::AffixFuzzer4(Some(datatypes::AffixFuzzer1 {
        single_float_optional: None,
        single_string_required: "d".into(),
        single_string_optional: None,
        many_floats_optional: Some(vec![4.0, 40.0, 400.0].into()),
        many_strings_required: vec!["7".into(), "8".into()],
        many_strings_optional: None,
        flattened_scalar: 45.0,
        almost_flattened_scalar: datatypes::FlattenedScalar { value: 45.0 },
        from_parent: None,
    }));

    let fuzzy5 = components::AffixFuzzer5(None);
    // let fuzzy5 = components::AffixFuzzer5(Some(datatypes::AffixFuzzer1 {
    //     single_float_optional: None,
    //     single_string_required: "d".into(),
    //     single_string_optional: None,
    //     many_floats_optional: Some(vec![4.0, 40.0, 400.0]),
    //     many_strings_required: vec!["7".into(), "8".into()],
    //     many_strings_optional: None,
    // }));

    let fuzzy6 = components::AffixFuzzer6(None);
    // let fuzzy6 = components::AffixFuzzer6 {
    //     single_optional: Some(datatypes::AffixFuzzer1 {
    //         // single_float_optional: None,
    //         single_string_required: "d".into(),
    //         // single_string_optional: None,
    //         //             many_floats_optional: Some(vec![4.0, 40.0, 400.0]),
    //         //             many_strings_required: vec!["7".into(), "8".into()],
    //         //             many_strings_optional: None,
    //     }),
    // };

    let fuzzy7_1 = components::AffixFuzzer7(None);
    let fuzzy7_2 = components::AffixFuzzer7(Some(vec![datatypes::AffixFuzzer1 {
        single_float_optional: None,
        single_string_required: "d".into(),
        single_string_optional: None,
        many_floats_optional: Some(vec![4.0, 40.0, 400.0].into()),
        many_strings_required: vec!["7".into(), "8".into()],
        many_strings_optional: None,
        flattened_scalar: 46.0,
        almost_flattened_scalar: datatypes::FlattenedScalar { value: 46.0 },
        from_parent: Some(false),
    }]));

    let fuzzy8_1 = components::AffixFuzzer8(None);
    let fuzzy8_2 = components::AffixFuzzer8(Some(1.0));

    let fuzzy9_1 = components::AffixFuzzer9("b".into());
    let fuzzy9_2 = components::AffixFuzzer9("a".into());

    let fuzzy10_1 = components::AffixFuzzer10(None);
    let fuzzy10_2 = components::AffixFuzzer10(Some("a".into()));

    let fuzzy11_1 = components::AffixFuzzer11(Some(vec![1.0, 10.0].into()));
    let fuzzy11_2 = components::AffixFuzzer11(Some(vec![2.0, 20.0, 200.0].into()));

    let fuzzy12_1 = components::AffixFuzzer12(vec!["1".into(), "10".into()]);
    let fuzzy12_2 = components::AffixFuzzer12(vec!["20".into(), "200".into(), "2000".into()]);

    let fuzzy13_1 = components::AffixFuzzer13(None);
    let fuzzy13_2 = components::AffixFuzzer13(Some(vec![
        "30".into(),
        "300".into(),
        "3000".into(),
        "30000".into(),
    ]));

    let fuzzy14_1 = components::AffixFuzzer14(datatypes::AffixFuzzer3::Degrees(90.0));
    let fuzzy14_2 = components::AffixFuzzer14(datatypes::AffixFuzzer3::EmptyVariant);

    let fuzzy15_1 = components::AffixFuzzer15(None);
    let fuzzy15_2 = components::AffixFuzzer15(Some(datatypes::AffixFuzzer3::Degrees(90.0)));

    let fuzzy16_1 = components::AffixFuzzer16(vec![
        datatypes::AffixFuzzer3::Degrees(45.0), //
    ]);
    let fuzzy16_2 = components::AffixFuzzer16(vec![
        datatypes::AffixFuzzer3::Degrees(20.0), //
        datatypes::AffixFuzzer3::EmptyVariant,  //
        datatypes::AffixFuzzer3::Degrees(30.0), //
    ]);

    let fuzzy17_1 = components::AffixFuzzer17(None);
    let fuzzy17_2 = components::AffixFuzzer17(Some(vec![
        datatypes::AffixFuzzer3::Degrees(20.0), //
        datatypes::AffixFuzzer3::Degrees(30.0), //
    ]));

    let fuzzy18_1 = components::AffixFuzzer18(None);
    let fuzzy18_2 = components::AffixFuzzer18(Some(vec![
        datatypes::AffixFuzzer4::SingleRequired(datatypes::AffixFuzzer3::Craziness(vec![
            datatypes::AffixFuzzer1 {
                single_float_optional: None,
                single_string_required: "d".into(),
                single_string_optional: None,
                many_floats_optional: Some(vec![4.0, 40.0, 400.0].into()),
                many_strings_required: vec!["7".into(), "8".into()],
                many_strings_optional: None,
                flattened_scalar: 46.0,
                almost_flattened_scalar: datatypes::FlattenedScalar { value: 46.0 },
                from_parent: Some(true),
            },
        ])), //
        datatypes::AffixFuzzer4::SingleRequired(datatypes::AffixFuzzer3::Degrees(30.0)), //
    ]));
    let fuzzy18_3 = components::AffixFuzzer18(Some(vec![
        datatypes::AffixFuzzer4::ManyRequired(vec![
            datatypes::AffixFuzzer3::Degrees(45.0), //
            datatypes::AffixFuzzer3::Craziness(vec![datatypes::AffixFuzzer1 {
                single_float_optional: Some(3.0),
                single_string_required: "c".into(),
                single_string_optional: Some("c".into()),
                many_floats_optional: Some(vec![3.0, 30.0, 300.0].into()),
                many_strings_required: vec!["5".into(), "6".into()],
                many_strings_optional: Some(vec!["50".into(), "60".into()]),
                flattened_scalar: 44.0,
                almost_flattened_scalar: datatypes::FlattenedScalar { value: 44.0 },
                from_parent: None,
            }]),
        ]), //
    ]));

    let fuzzy19_1 = components::AffixFuzzer19(datatypes::AffixFuzzer5 {
        single_optional_union: Some(datatypes::AffixFuzzer4::ManyRequired(vec![
            datatypes::AffixFuzzer3::Degrees(45.0), //
            datatypes::AffixFuzzer3::Craziness(vec![datatypes::AffixFuzzer1 {
                single_float_optional: Some(3.0),
                single_string_required: "c".into(),
                single_string_optional: Some("c".into()),
                many_floats_optional: Some(vec![3.0, 30.0, 300.0].into()),
                many_strings_required: vec!["5".into(), "6".into()],
                many_strings_optional: Some(vec!["50".into(), "60".into()]),
                flattened_scalar: 44.0,
                almost_flattened_scalar: datatypes::FlattenedScalar { value: 44.0 },
                from_parent: None,
            }]),
        ])), //
    });

    let fuzzy20 = components::AffixFuzzer20(datatypes::AffixFuzzer20 {
        p: datatypes::PrimitiveComponent(17),
        s: datatypes::StringComponent("fuzz".to_owned().into()),
    });

    let fuzzy21 = components::AffixFuzzer21(datatypes::AffixFuzzer21 {
        single_half: f16::from_f32(123.4),
        many_halves: vec![f16::from_f32(123.4), f16::from_f32(567.8)].into(),
    });

    let fuzzy22_1 = components::AffixFuzzer22(Some(datatypes::AffixFuzzer22 {
        fixed_sized_native: [1, 2, 3, 4],
    }));

    let fuzzy22_2 = components::AffixFuzzer22(None);

    {
        let arch = AffixFuzzer1::new(
            fuzzy1.clone(), //
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
            fuzzy14_1.clone(),
            fuzzy15_1.clone(),
            fuzzy16_2.clone(),
            fuzzy17_2.clone(),
            fuzzy18_2.clone(),
            fuzzy19_1.clone(),
            fuzzy20.clone(),
            fuzzy21.clone(),
            fuzzy22_1.clone(),
        );

        #[rustfmt::skip]
    let expected_extensions: HashMap<_, _> = [
        ("fuzz1001", vec!["rerun.testing.components.AffixFuzzer1", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1002", vec!["rerun.testing.components.AffixFuzzer2", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1003", vec!["rerun.testing.components.AffixFuzzer3", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1004", vec!["rerun.testing.components.AffixFuzzer4", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1005", vec!["rerun.testing.components.AffixFuzzer5", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1006", vec!["rerun.testing.components.AffixFuzzer6", "rerun.testing.datatypes.AffixFuzzer1"]),
    ]
    .into();

        eprintln!("arch = {arch:#?}");
        let serialized = arch.to_arrow().unwrap();
        for (field, array) in &serialized {
            // NOTE: Keep those around please, very useful when debugging.
            // eprintln!("field = {field:#?}");
            // eprintln!("array = {array:#?}");
            if field.name == "rerun.testing.components.AffixFuzzer21" {
                // TODO(jleibs): Fields that contain Float16 apparently don't supported fmt
                // https://github.com/rerun-io/re_arrow2/blob/33a32000001df800e4840d92c33b03e7007311e1/src/array/primitive/fmt.rs#L39
                eprintln!("{} = Can't be printed (float16 not supported)", field.name);
            } else {
                eprintln!("{} = {array:#?}", field.name);
            }

            // TODO(cmc): Re-enable extensions and these assertions once `arrow2-convert`
            // has been fully replaced.
            if false {
                util::assert_extensions(
                    &**array,
                    expected_extensions[field.name.as_str()].as_slice(),
                );
            }
        }

        let deserialized = AffixFuzzer1::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(arch, deserialized);
    }

    {
        let arch = AffixFuzzer2::new(
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
            [fuzzy14_1.clone(), fuzzy14_2.clone(), fuzzy14_2.clone()],
            [fuzzy15_1.clone(), fuzzy15_2.clone(), fuzzy15_1.clone()],
            [fuzzy16_1.clone(), fuzzy16_2.clone(), fuzzy16_1.clone()],
            [fuzzy17_1.clone(), fuzzy17_2.clone(), fuzzy17_1.clone()],
            [fuzzy18_1.clone(), fuzzy18_2.clone(), fuzzy18_3.clone()],
            [fuzzy22_1.clone(), fuzzy22_2.clone(), fuzzy22_1.clone()],
        );

        #[rustfmt::skip]
    let expected_extensions: HashMap<_, _> = [
        ("fuzz1101", vec!["rerun.testing.components.AffixFuzzer1", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1102", vec!["rerun.testing.components.AffixFuzzer2", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1103", vec!["rerun.testing.components.AffixFuzzer3", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1104", vec!["rerun.testing.components.AffixFuzzer4", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1105", vec!["rerun.testing.components.AffixFuzzer5", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz1106", vec!["rerun.testing.components.AffixFuzzer6", "rerun.testing.datatypes.AffixFuzzer1"]),
    ]
    .into();

        eprintln!("arch = {arch:#?}");
        let serialized = arch.to_arrow().unwrap();
        for (field, array) in &serialized {
            // NOTE: Keep those around please, very useful when debugging.
            // eprintln!("field = {field:#?}");
            // eprintln!("array = {array:#?}");
            if field.name == "rerun.testing.components.AffixFuzzer21" {
                // TODO(jleibs): Fields that contain Float16 apparently don't supported fmt
                // https://github.com/rerun-io/re_arrow2/blob/33a32000001df800e4840d92c33b03e7007311e1/src/array/primitive/fmt.rs#L39
                eprintln!("{} = Can't be printed (float16 not supported)", field.name);
            } else {
                eprintln!("{} = {array:#?}", field.name);
            }

            // TODO(cmc): Re-enable extensions and these assertions once `arrow2-convert`
            // has been fully replaced.
            if false {
                util::assert_extensions(
                    &**array,
                    expected_extensions[field.name.as_str()].as_slice(),
                );
            }
        }

        let deserialized = AffixFuzzer2::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(arch, deserialized);
    }

    {
        let arch = AffixFuzzer3::new()
            .with_fuzz2001(fuzzy1.clone())
            .with_fuzz2003(fuzzy3.clone())
            .with_fuzz2005(fuzzy5.clone())
            .with_fuzz2007(fuzzy7_1.clone())
            .with_fuzz2009(fuzzy9_1.clone())
            .with_fuzz2011(fuzzy11_1.clone())
            .with_fuzz2013(fuzzy13_1.clone())
            .with_fuzz2015(fuzzy15_1.clone())
            .with_fuzz2016(fuzzy16_1.clone())
            .with_fuzz2017(fuzzy17_1.clone())
            .with_fuzz2018(fuzzy18_1.clone());

        #[rustfmt::skip]
    let expected_extensions: HashMap<_, _> = [
        ("fuzz2001", vec!["rerun.testing.components.AffixFuzzer1", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2002", vec!["rerun.testing.components.AffixFuzzer2", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2003", vec!["rerun.testing.components.AffixFuzzer3", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2004", vec!["rerun.testing.components.AffixFuzzer4", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2005", vec!["rerun.testing.components.AffixFuzzer5", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2006", vec!["rerun.testing.components.AffixFuzzer6", "rerun.testing.datatypes.AffixFuzzer1"]),
    ]
    .into();

        eprintln!("arch = {arch:#?}");
        let serialized = arch.to_arrow().unwrap();
        for (field, array) in &serialized {
            // NOTE: Keep those around please, very useful when debugging.
            // eprintln!("field = {field:#?}");
            // eprintln!("array = {array:#?}");
            if field.name == "rerun.testing.components.AffixFuzzer21" {
                // TODO(jleibs): Fields that contain Float16 apparently don't supported fmt
                // https://github.com/rerun-io/re_arrow2/blob/33a32000001df800e4840d92c33b03e7007311e1/src/array/primitive/fmt.rs#L39
                eprintln!("{} = Can't be printed (float16 not supported)", field.name);
            } else {
                eprintln!("{} = {array:#?}", field.name);
            }

            // TODO(cmc): Re-enable extensions and these assertions once `arrow2-convert`
            // has been fully replaced.
            if false {
                util::assert_extensions(
                    &**array,
                    expected_extensions[field.name.as_str()].as_slice(),
                );
            }
        }

        let deserialized = AffixFuzzer3::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(arch, deserialized);
    }

    {
        let arch = AffixFuzzer4::new()
            .with_fuzz2102([fuzzy2.clone(), fuzzy2.clone(), fuzzy2.clone()])
            .with_fuzz2104([fuzzy4.clone(), fuzzy4.clone(), fuzzy4.clone()])
            .with_fuzz2106([fuzzy6.clone(), fuzzy6.clone(), fuzzy6.clone()])
            .with_fuzz2108([fuzzy8_1.clone(), fuzzy8_2.clone(), fuzzy8_1.clone()])
            .with_fuzz2110([fuzzy10_1.clone(), fuzzy10_2.clone(), fuzzy10_1.clone()])
            .with_fuzz2112([fuzzy12_1.clone(), fuzzy12_2.clone(), fuzzy12_1.clone()])
            .with_fuzz2114([fuzzy14_1.clone(), fuzzy14_1.clone(), fuzzy14_1.clone()])
            .with_fuzz2115([fuzzy15_1.clone(), fuzzy15_2.clone(), fuzzy15_1.clone()])
            .with_fuzz2116([fuzzy16_1.clone(), fuzzy16_2.clone(), fuzzy16_1.clone()])
            .with_fuzz2117([fuzzy17_1.clone(), fuzzy17_2.clone(), fuzzy17_1.clone()])
            .with_fuzz2118([fuzzy18_1.clone(), fuzzy18_2.clone(), fuzzy18_3.clone()]);

        #[rustfmt::skip]
    let expected_extensions: HashMap<_, _> = [
        ("fuzz2101", vec!["rerun.testing.components.AffixFuzzer1", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2102", vec!["rerun.testing.components.AffixFuzzer2", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2103", vec!["rerun.testing.components.AffixFuzzer3", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2104", vec!["rerun.testing.components.AffixFuzzer4", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2105", vec!["rerun.testing.components.AffixFuzzer5", "rerun.testing.datatypes.AffixFuzzer1"]),
        ("fuzz2106", vec!["rerun.testing.components.AffixFuzzer6", "rerun.testing.datatypes.AffixFuzzer1"]),
    ]
    .into();

        eprintln!("arch = {arch:#?}");
        let serialized = arch.to_arrow().unwrap();
        for (field, array) in &serialized {
            // NOTE: Keep those around please, very useful when debugging.
            // eprintln!("field = {field:#?}");
            // eprintln!("array = {array:#?}");
            if field.name == "rerun.testing.components.AffixFuzzer21" {
                // TODO(jleibs): Fields that contain Float16 apparently don't supported fmt
                // https://github.com/rerun-io/re_arrow2/blob/33a32000001df800e4840d92c33b03e7007311e1/src/array/primitive/fmt.rs#L39
                eprintln!("{} = Can't be printed (float16 not supported)", field.name);
            } else {
                eprintln!("{} = {array:#?}", field.name);
            }

            // TODO(cmc): Re-enable extensions and these assertions once `arrow2-convert`
            // has been fully replaced.
            if false {
                util::assert_extensions(
                    &**array,
                    expected_extensions[field.name.as_str()].as_slice(),
                );
            }
        }

        let deserialized = AffixFuzzer4::from_arrow(serialized).unwrap();
        similar_asserts::assert_eq!(arch, deserialized);
    }
}

mod util;
