pub mod archetypes;
pub mod components;
pub mod datatypes;

/// Large struct used for benchmarking.
pub type LargeStruct = components::AffixFuzzer1;

/// Builds len instances of [`LargeStruct`]
pub fn build_some_large_structs(len: usize) -> Vec<LargeStruct> {
    (0..len)
        .map(|i| {
            components::AffixFuzzer1(datatypes::AffixFuzzer1 {
                single_float_optional: Some(i as f32),
                single_string_required: format!("label{i}").into(),
                single_string_optional: Some(format!("label{i}").into()),
                many_floats_optional: None,
                many_strings_required: ["hello", "friend", "let's", "test!"]
                    .into_iter()
                    .take(i % 5)
                    .map(crate::ArrowString::from)
                    .collect(),
                many_strings_optional: None,
                flattened_scalar: i as f32,
                almost_flattened_scalar: datatypes::FlattenedScalar { value: i as f32 },
                from_parent: match i % 3 {
                    0 => Some(true),
                    1 => Some(false),
                    2 => None,
                    _ => unreachable!(),
                },
            })
        })
        .collect()
}
