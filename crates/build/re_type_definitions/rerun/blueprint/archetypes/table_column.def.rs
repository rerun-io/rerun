// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Blueprint for a column used by a table or card layout.
///
/// This archetype is stored at the layout-specific path for its source column.
/// The source is the final [`rerun::blueprint::components::ColumnName`] part of that path.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct TableColumn {
    /// The name shown for the column.
    ///
    /// If unset, the name is inferred from the source column.
    #[rerun(optional)]
    pub name: Option<rerun::components::Name>,

    /// Whether the column's values can be edited.
    ///
    /// If unset, editing is disabled.
    /// Edits requires a remote table with a column marked by `rerun:is_table_index` metadata and write permission.
    /// ⚠ Currently only boolean values are supported.
    #[rerun(optional)]
    pub editable: Option<rerun::blueprint::components::Editable>,

    /// Whether the column is visible in this layout.
    ///
    /// If unset, the enclosing layout determines visibility.
    /// Table layouts use the viewer default for the source column.
    /// Card layouts show sources listed in `field_order` and hide unlisted sources.
    #[rerun(optional)]
    pub visible: Option<rerun::components::Visible>,

    /// How to render the column's values.
    ///
    /// If unset or `Auto`, the viewer infers the renderer from the component or Arrow datatype.
    #[rerun(optional)]
    pub cell_kind: Option<rerun::blueprint::components::TableCellKind>,
}
