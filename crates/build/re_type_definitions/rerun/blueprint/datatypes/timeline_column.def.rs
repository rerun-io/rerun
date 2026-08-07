// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A timeline column in a table.
#[rerun::rerun_type]
#[python(aliases = "datatypes.Utf8Like")]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq, Hash))]
#[rerun(state = "unstable")]
pub struct TimelineColumn {
    /// Is this column visible?
    ///
    /// Defaults to true.
    pub visible: rerun::datatypes::Bool,

    /// Which timeline is this?
    pub timeline: rerun::datatypes::Utf8,
}
