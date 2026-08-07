// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The layout share of a row in the container.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "npt.ArrayLike")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct RowShare {
    /// The layout share of a row in the container.
    pub share: rerun::datatypes::Float32,
}
