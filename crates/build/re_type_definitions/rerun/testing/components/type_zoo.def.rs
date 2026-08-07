// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

// The "type zoo": a hand-authored menagerie of exotic type combinations
// (nullable fields, arrays, unions, enums, transparent/nested datatypes,
// fixed-size arrays, …) used to exercise every corner of the codegen.
// Despite the `AffixFuzzer*` names, nothing here is randomly fuzzed.

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer1 {
    pub single_required: rerun::testing::datatypes::MixedFields,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer10 {
    pub single_string_optional: Option<String>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer11 {
    pub many_floats_optional: Option<Vec<f32>>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer12 {
    pub many_strings_required: Vec<String>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer13 {
    pub many_strings_optional: Option<Vec<String>>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer14 {
    pub single_required_union: rerun::testing::datatypes::ScalarUnion,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer15 {
    pub single_optional_union: Option<rerun::testing::datatypes::ScalarUnion>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer16 {
    pub many_required_unions: Vec<rerun::testing::datatypes::ScalarUnion>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer17 {
    pub many_optional_unions: Option<Vec<rerun::testing::datatypes::ScalarUnion>>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer18 {
    pub many_optional_unions: Option<Vec<rerun::testing::datatypes::NestedUnion>>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer19 {
    pub just_a_table_nothing_shady: rerun::testing::datatypes::OptionalUnionTable,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer2 {
    pub single_required: rerun::testing::datatypes::MixedFields,
}

#[rerun::rerun_type]
#[rust(derive(Default, Eq, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer20 {
    pub nested_transparent: rerun::testing::datatypes::PrimitiveAndString,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer21 {
    pub nested_halves: rerun::testing::datatypes::Float16Fields,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer22 {
    pub nullable_nested_array: Option<rerun::testing::datatypes::FixedSizeBytes>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer23 {
    pub multi_enum: Option<rerun::testing::datatypes::MultiEnum>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer3 {
    pub single_required: rerun::testing::datatypes::MixedFields,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer4 {
    pub single_optional: Option<rerun::testing::datatypes::MixedFields>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer5 {
    pub single_optional: Option<rerun::testing::datatypes::MixedFields>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer6 {
    pub single_optional: Option<rerun::testing::datatypes::MixedFields>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer7 {
    pub many_optional: Option<Vec<rerun::testing::datatypes::MixedFields>>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer8 {
    pub single_float_optional: Option<f32>,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "stable")]
pub struct AffixFuzzer9 {
    pub single_string_required: String,
}

#[rerun::rerun_type]
#[rust(derive(Default, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct ManyVec3 {
    pub nested_array_of_structs: rerun::testing::datatypes::ManyVec3,
}

// TODO(cmc): the ugly bug we need to take care of at some point
// #[rerun::rerun_type]
// #[rust(derive(Default, PartialEq))]
// pub struct AffixFuzzer14 {
//     pub many_transparent_optionals: Option<rerun::testing::datatypes::TransparentOptionalFloat>,
// }
