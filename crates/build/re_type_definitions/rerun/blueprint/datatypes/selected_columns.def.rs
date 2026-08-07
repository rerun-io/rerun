// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// List of selected columns in a dataframe.
#[rerun::rerun_type]
#[python(
    aliases = "Sequence[blueprint_datatypes.ComponentColumnSelectorLike | datatypes.Utf8Like]"
)]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "unstable")]
pub struct SelectedColumns {
    // pub row_id: rerun::datatypes::Bool, // TODO(#9921): add support for showing Row ID in UI
    /// The time columns to include
    pub time_columns: Vec<rerun::datatypes::Utf8>,

    /// The component columns to include
    pub component_columns: Vec<rerun::blueprint::datatypes::ComponentColumnSelector>,
}
