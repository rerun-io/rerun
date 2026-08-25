// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Blueprint for displaying table records as rows and columns.
///
/// This archetype is stored at the entity `/table/layouts/table`.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct TableLayout {
    /// Source columns to show first, in display order.
    ///
    /// Unmentioned columns retain the viewer defaults and follow in default order.
    /// Each source column may appear at most once.
    #[rerun(optional)]
    pub column_order: Option<Vec<rerun::blueprint::components::ColumnName>>,
    // TODO(andreas): add `auto_visible_columns`.
    // Whether unmentioned source columns use the viewer's default visibility.
    //
    // If false, only columns referenced by `column_order` are visible by default.
    // A referenced column can still be hidden explicitly with [`rerun::blueprint::archetypes::TableColumn`].
    // If unset, this defaults to true.
    //#[rerun(optional)]
    //pub auto_visible_columns: Option<rerun::components::Visible>,
}
