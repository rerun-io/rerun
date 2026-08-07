// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

// The "type zoo": a hand-authored menagerie of exotic type combinations
// (nullable fields, arrays, unions, enums, transparent/nested datatypes,
// fixed-size arrays, …) used to exercise every corner of the codegen.
// The `components`/`archetypes` counterparts still use the historical
// `AffixFuzzer*` names; nothing here is randomly fuzzed.

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "stable")]
pub struct FixedSizeBytes {
    pub fixed_sized_native: [u8; 4],
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct FlattenedScalar {
    pub value: f32,
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct Float16Fields {
    #[rerun(override_type = "float16")]
    pub single_half: rerun::f16,

    #[rerun(override_type = "float16")]
    pub many_halves: Vec<rerun::f16>,
}

/// A fixed-size array of arrays — exercises nested fixed-size lists in Arrow.
#[rerun::rerun_type]
#[arrow(transparent)]
#[rust(derive(Default, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "C")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct ManyVec3 {
    pub triples: [[f32; 3]; 2],
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct MixedFields {
    pub single_float_optional: Option<f32>,

    pub single_string_required: String,

    pub single_string_optional: Option<String>,

    pub many_floats_optional: Option<Vec<f32>>,

    pub many_strings_required: Vec<String>,

    pub many_strings_optional: Option<Vec<String>>,

    pub flattened_scalar: f32,

    pub almost_flattened_scalar: rerun::testing::datatypes::FlattenedScalar,

    pub from_parent: Option<bool>,
}

#[rerun::rerun_type]
#[repr(i8)]
#[rust(derive(PartialEq))]
#[rerun(state = "stable")]
pub enum NestedUnion {
    single_required(rerun::testing::datatypes::ScalarUnion) = 1,

    many_required(Vec<rerun::testing::datatypes::ScalarUnion>) = 2,
    //many_optional(Option<Vec<rerun::testing::datatypes::ScalarUnion>>) = 3, // Nullable fields on unions are not supported.
}

#[rerun::rerun_type]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct OptionalUnionTable {
    pub single_optional_union: Option<rerun::testing::datatypes::NestedUnion>,
}

#[rerun::rerun_type]
#[rust(derive(Default, Eq, PartialEq))]
#[rerun(state = "stable")]
pub struct PrimitiveAndString {
    pub p: rerun::testing::datatypes::PrimitiveComponent,

    pub s: rerun::testing::datatypes::StringComponent,
}

#[rerun::rerun_type]
#[repr(i8)]
#[rust(derive(PartialEq))]
#[rerun(state = "stable")]
pub enum ScalarUnion {
    degrees(f32) = 1,
    //radians(Option<f32>) = 5, // Nullable fields on unions are not supported.
    craziness(Vec<rerun::testing::datatypes::MixedFields>) = 2,

    fixed_size_shenanigans([f32; 3]) = 3,

    empty_variant = 4,
}

#[rerun::rerun_type]
#[arrow(transparent)]
#[rust(derive(Default, PartialEq))]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct TransparentOptionalFloat {
    pub single_float_optional: Option<f32>,
}
