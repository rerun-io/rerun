// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Overrides the component a table column's values are shown as.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq, Hash))]
#[rerun(state = "unstable")]
pub struct ColumnType {
    /// The component identifier, e.g. `GridMap:colormap`.
    ///
    /// Some components have a UI specific to the archetype field they are logged as.
    /// If not set, the component identifier of the source column is used.
    pub component: Option<String>,

    /// The fully qualified component type, e.g. `rerun.components.Flag`.
    ///
    /// If not set, the component type of the source column is used, or the Viewer infers one from the column's datatype.
    pub component_type_name: Option<String>,
}
