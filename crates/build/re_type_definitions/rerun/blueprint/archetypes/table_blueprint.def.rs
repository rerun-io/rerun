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
    /// meaning that the regular structure of [archetypes.ViewportBlueprint], and [archetypes.ViewBlueprint] structure applies.
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
