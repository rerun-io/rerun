// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Linear speed, used for translation speed for example.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "npt.ArrayLike")]
#[rerun(state = "stable")]
pub struct LinearSpeed {
    /// Speed value in units of length per unit of time.
    pub speed: rerun::datatypes::Float64,
}
