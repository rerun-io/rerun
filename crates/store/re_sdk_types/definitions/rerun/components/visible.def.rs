// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Whether the container, view, entity or instance is currently visible.
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "bool")]
#[rerun(state = "stable")]
#[rust(derive(Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
pub struct Visible {
    pub visible: rerun::datatypes::Bool,
}
