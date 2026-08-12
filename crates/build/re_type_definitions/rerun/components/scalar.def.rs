// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A scalar value, encoded as a 64-bit floating point.
///
/// Used for time series plots.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.NDArray[np.float64]")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct Scalar {
    pub value: rerun::datatypes::Float64,
}
