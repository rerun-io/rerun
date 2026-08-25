// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Blueprint for previewing recording references from a table column.
///
/// This archetype is stored on the same layout-specific column entity as [`rerun::blueprint::archetypes::TableColumn`].
/// Table columns use `/table/layouts/table/columns/{column_name}`.
/// Card fields use `/table/layouts/cards/fields/{column_name}`.
/// Shared preview configuration is stored as [`rerun::blueprint::archetypes::PreviewsConfig`] at `/table`.
///
/// A column uses this payload only when its `TableCellKind::Preview` is set.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct TableColumnPreview {
    /// The views rendered for the preview, in display order.
    ///
    /// Each [`rerun::blueprint::components::IncludedContent`] must reference a [`rerun::blueprint::archetypes::ViewBlueprint`] at `/view/{view_id}`.
    /// View contents, properties, defaults, and overrides remain at their regular blueprint paths.
    #[rerun(required)]
    pub views: Vec<rerun::blueprint::components::IncludedContent>,
}
