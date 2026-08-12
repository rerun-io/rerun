// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration for the filter is not null feature of the dataframe view.
#[rerun::rerun_type]
#[python(aliases = "blueprint_datatypes.ComponentColumnSelectorLike")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "unstable")]
pub struct FilterIsNotNull {
    /// Whether the filter by event feature is active.
    pub active: rerun::datatypes::Bool,

    /// The column used when the filter by event feature is used.
    pub column: rerun::blueprint::datatypes::ComponentColumnSelector,
}
