// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The length of an axis in local units of the space.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.ArrayLike")]
#[rust(derive(Copy, PartialEq, PartialOrd))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct AxisLength {
    pub length: rerun::datatypes::Float32,
}
