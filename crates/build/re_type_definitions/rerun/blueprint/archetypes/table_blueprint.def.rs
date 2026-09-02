// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Blueprint for configuring the styling of a table.
///
/// The table blueprint as a whole is distributed across these entity paths:
/// * `/table` for this archetype and [`rerun::blueprint::archetypes::PreviewsConfig`].
/// * `/table/layouts/table` for [`rerun::blueprint::archetypes::TableLayout`].
/// * `/table/layouts/table/columns/{column_name}` for table [`rerun::blueprint::archetypes::TableColumn`] archetypes and per-column options such as [`rerun::blueprint::archetypes::TableColumnPreview`].
/// * `/table/layouts/cards` for [`rerun::blueprint::archetypes::CardLayout`].
/// * `/table/layouts/cards/fields/{column_name}` for card [`rerun::blueprint::archetypes::TableColumn`] archetypes and per-field options such as [`rerun::blueprint::archetypes::TableColumnPreview`].
/// * `/view/{view_id}` for preview [`rerun::blueprint::archetypes::ViewBlueprint`] definitions.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct TableBlueprint {
    /// The currently selected layout.
    ///
    /// If unset, defaults to card layout if available.
    /// `Cards` falls back to table layout when no [`rerun::blueprint::archetypes::CardLayout`] is configured.
    #[rerun(optional)]
    pub layout: Option<rerun::blueprint::components::TableLayoutKind>,
    // TODO(andreas): Reject `Cards` without a configured card layout in the ergonomic API.
    // TODO(andreas): Add automatic column display name formatting.
    // TODO(andreas): Add persisted column sorting.
    // TODO(andreas): Add persisted column filters.
}
