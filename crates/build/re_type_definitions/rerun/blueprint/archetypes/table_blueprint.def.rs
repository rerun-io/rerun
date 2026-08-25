// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Blueprint for configuring the styling of a table.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct TableBlueprint {
    /// The name of the column that contains recording URIs for segment previews.
    ///
    /// Every row can at most preview a single segment.
    ///
    /// For the preview, the rest of the blueprint data is read it as it would be with regular recording blueprints,
    /// meaning that the regular structure of [`rerun::blueprint::archetypes::ViewportBlueprint`], and [`rerun::blueprint::archetypes::ViewBlueprint`] structure applies.
    /// However, this mostly ignores layout container types as well as automatic spawning.
    ///
    /// If unset, defaults to the first URL column in the table that points to the same Rerun server
    #[rerun(optional)]
    pub segment_preview_column: Option<rerun::blueprint::components::ColumnName>,

    /// The name of the boolean column used for flag/annotation toggles.
    ///
    /// Must be set for flagging to be available. The named column must exist in the
    /// table and be of boolean type.
    /// Additionally, the table must be remote and have another column with
    /// `rerun:is_table_index` metadata since flag changes are persisted to the server
    /// via upsert.
    #[rerun(optional)]
    pub flag_column: Option<rerun::blueprint::components::ColumnName>,

    /// The name of the column to use as the card title in grid view.
    ///
    /// If unset, the first visible string column is used as the title.
    #[rerun(optional)]
    pub grid_view_card_title: Option<rerun::blueprint::components::ColumnName>,

    /// The name of the column containing URLs to open when a card is clicked in grid view.
    ///
    /// If unset, defaults to the segment preview column.
    #[rerun(optional)]
    pub url_column: Option<rerun::blueprint::components::ColumnName>,
}

/// Blueprint for configuring the styling of a table.
///
/// TODO(RR-4810): This is not yet in use!
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
pub struct TableBlueprintV2 {
    /// The layout shown when the table is first opened and no user selection has been persisted.
    ///
    /// If unset, defaults to card layout if available.
    /// `Cards` falls back to table layout when no [`rerun::blueprint::archetypes::CardLayout`] is configured.
    #[rerun(optional)]
    pub default_layout: Option<rerun::blueprint::components::TableLayoutKind>,
    // TODO(andreas): Reject `Cards` without a configured card layout in the ergonomic API.
    // TODO(andreas): Add automatic column display name formatting.
    // TODO(andreas): Add persisted column sorting.
    // TODO(andreas): Add persisted column filters.
}
