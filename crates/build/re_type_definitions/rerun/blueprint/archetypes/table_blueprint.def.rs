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
    /// Write one condition per filter instead of joining them with `AND`.
    ///
    /// Users cannot write SQL in the viewer.
    /// They add filters with "Filter" in a column header's menu, which only writes the forms below.
    /// The viewer shows these forms on a single column `"c"` as filters the user can edit.
    /// Any other expression shows as a `Custom "c" filter` chip, which the user can only remove and whose SQL only shows on hover:
    /// * Numbers: `"c" = 7`, `"c" < 7`, `"c" <= 7`, `"c" > 7`, `"c" >= 7`.
    ///   For a 32-bit float column, write the value as `CAST(0.1 AS REAL)`.
    /// * Strings: `"c" ILIKE '%text%'` for contains, `'text%'` for starts with, `'%text'` for ends with.
    ///   Put a backslash before a literal `%`, `_` or backslash, and follow the pattern with an ESCAPE clause that names a quoted backslash.
    /// * Booleans: `"c" = true`, `"c" = false`, and `"c" IS NULL` for a nullable column.
    /// * Timestamps: `"c" < TIMESTAMPTZ '2026-01-01T00:00:00Z'` for before, `>=` for after, and both joined with `AND` for between.
    ///   Relative ranges have the same shape, such as `"c" >= now() - INTERVAL '24 hours' AND "c" < now()` for the last 24 hours,
    ///   or `"c" >= date_trunc('day', now()) AND "c" < date_trunc('day', now()) + INTERVAL '1 day'` for today.
    ///   Use `'week'` with `INTERVAL '7 days'` for this week, and shift both bounds back for yesterday or last week.
    ///   For an `Int64` column of nanoseconds, use `to_timestamp_nanos("c")`.
    /// * Lists: any form above on `x` inside `any_match("c", x -> …)`, true when any element is.
    ///
    /// Wrapping a form in `(…) IS NOT TRUE` gives its opposite, such as not equal or does not contain.
    #[rerun(optional)]
    pub filters: Option<Vec<rerun::blueprint::components::SqlFilterExpression>>,
    // TODO(andreas): Reject `Cards` without a configured card layout in the ergonomic API.
    // TODO(andreas): Add persisted column sorting.
}
