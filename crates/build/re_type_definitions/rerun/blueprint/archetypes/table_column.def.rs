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
    /// If unset, the column is editable if its field in the table schema has `rerun:is_editable` metadata.
    /// Edits require a remote table with a column marked by `rerun:is_table_index` metadata and write permission.
    /// A column can only be edited when its component type has an editor in the Viewer.
    #[rerun(optional)]
    pub editable: Option<rerun::blueprint::components::Editable>,

    /// Whether the column is visible in this layout.
    ///
    /// If unset, the enclosing layout determines visibility.
    /// Columns listed in the enclosing layout's `column_order` or `field_order` are shown.
    /// Unlisted columns use the viewer default for table layouts and are hidden in card layouts.
    #[rerun(optional)]
    pub visible: Option<rerun::components::Visible>,

    /// Overrides the component the column's values are shown as.
    ///
    /// This picks the component UI for every value of the column.
    /// The column's Arrow datatype must match the datatype of the component type.
    /// If unset, the component of the source column is used, or the Viewer infers one from the column's datatype.
    #[rerun(optional)]
    pub column_type: Option<rerun::blueprint::components::ColumnType>,
}
