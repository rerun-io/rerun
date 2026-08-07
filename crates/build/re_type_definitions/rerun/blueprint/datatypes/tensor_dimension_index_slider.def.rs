// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Defines a slider for the index of some dimension.
#[rerun::rerun_type]
#[python(aliases = "int")]
#[python(array_aliases = "npt.ArrayLike")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, Copy, Hash, PartialEq, Eq))]
#[rerun(state = "unstable")]
pub struct TensorDimensionIndexSlider {
    /// The dimension number.
    pub dimension: u32,
    // TODO(andreas): Range of the slider?
    // Full Range if not specified.
    //pub range: Option<[u32; 2]>,
}
