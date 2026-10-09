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

    /// Formatting for column names in table and card layouts.
    ///
    /// Defaults to compact formatting when unset.
    /// Explicit column display names take precedence.
    #[rerun(optional)]
    pub column_display_mode: Option<rerun::blueprint::components::ColumnDisplayMode>,

    /// Filters for the table's rows.
    ///
    /// A row in the table is only shown when *all* filters evaluate to true for it.
    /// Each filter only sees the values of a single row, so functions that combine rows, such as `sum` or `count`, are not allowed.
    ///
    /// The viewer shows an editable filter for a single column compared to a literal value, for example:
    /// * `"score" >= 3`
    /// * `"success" = true`
    /// * `"task" ILIKE '%pick%'`
    /// * `"created" >= TIMESTAMP '2026-01-01T00:00:00Z'`
    ///
    /// For a list column, the comparison goes in `any_match("column", x -> …)`.
    /// Other expressions still filter the table, and the viewer shows them as SQL text.
    #[rerun(optional)]
    pub filters: Option<Vec<rerun::blueprint::components::SqlFilterExpression>>,
    // TODO(andreas): Reject `Cards` without a configured card layout in the ergonomic API.
    // TODO(andreas): Add persisted column sorting.
}
