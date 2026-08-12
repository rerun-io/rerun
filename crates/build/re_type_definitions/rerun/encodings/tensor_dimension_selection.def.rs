// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Indexing a specific tensor dimension.
///
/// Selecting `dimension=2` and `index=42` is similar to doing `tensor[:, :, 42, :, :, …]` in numpy.
#[rerun::rerun_type]
#[rust(derive(Default, Copy, Hash, PartialEq, Eq))]
#[rerun(state = "stable")]
pub struct TensorDimensionIndexSelection {
    /// The dimension number to select.
    pub dimension: u32,

    /// The index along the dimension to use.
    pub index: u64,
}

/// Selection of a single tensor dimension.
#[rerun::rerun_type]
#[python(aliases = "int")]
#[python(array_aliases = "npt.ArrayLike")]
#[rust(derive(Default, Copy, Hash, PartialEq, Eq))]
#[rerun(state = "stable")]
pub struct TensorDimensionSelection {
    /// The dimension number to select.
    pub dimension: u32,

    /// Invert the direction of the dimension.
    pub invert: bool,
}
