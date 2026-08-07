// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Space between grid lines of one line to the next in scene units.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "npt.ArrayLike")]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct GridSpacing {
    /// Space between grid lines of one line to the next in scene units.
    pub distance: rerun::datatypes::Float32,
}
