// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Describe a component column to be selected in the dataframe view.
#[rerun::rerun_type]
#[arrow(transparent)]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq, Hash))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct ComponentColumnSelector {
    pub selector: rerun::blueprint::datatypes::ComponentColumnSelector,
}
