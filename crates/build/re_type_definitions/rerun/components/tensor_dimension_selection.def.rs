// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Specifies a concrete index on a tensor dimension.
#[rerun::rerun_type]
#[rust(derive(Hash, Copy, PartialEq, Eq, Default))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct TensorDimensionIndexSelection {
    pub selection: rerun::datatypes::TensorDimensionIndexSelection,
}

/// Specifies which dimension to use for height.
#[rerun::rerun_type]
#[rust(derive(Hash, Copy, PartialEq, Eq, Default))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct TensorHeightDimension {
    pub dimension: rerun::datatypes::TensorDimensionSelection,
}

/// Specifies which dimension to use for width.
#[rerun::rerun_type]
#[rust(derive(Hash, Copy, PartialEq, Eq, Default))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct TensorWidthDimension {
    pub dimension: rerun::datatypes::TensorDimensionSelection,
}
