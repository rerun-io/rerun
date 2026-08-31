// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Variance of a scalar measurement, i.e. σ², in the units of the value squared.
///
/// A value of `0` is a perfectly known value and draws no error band.
#[rerun::rerun_type]
#[docs(unreleased)]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.NDArray[np.float64]")]
#[rerun(state = "unstable")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
pub struct Variance {
    pub value: rerun::encodings::Float64,
}
