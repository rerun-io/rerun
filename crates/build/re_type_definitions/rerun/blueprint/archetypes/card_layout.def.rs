// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Blueprint for displaying table records as cards.
///
/// This archetype is stored at the entity `/table/layouts/cards`.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct CardLayout {
    /// The source column used for card titles.
    ///
    /// If unset, the first visible string column is used as the title.
    #[rerun(optional)]
    pub title: Option<rerun::blueprint::components::ColumnName>,

    /// The source column containing the target opened when a card is activated.
    ///
    /// If unset, the first configured preview field is used, then the first inferred URL column.
    #[rerun(optional)]
    pub link: Option<rerun::blueprint::components::ColumnName>,

    /// Source columns visible by default in each card, in display order.
    ///
    /// Unlisted fields are hidden unless their [`rerun::blueprint::archetypes::TableColumn`] visibility overrides the default.
    /// Each source column may appear at most once.
    ///
    /// Fields with `TableCellKind::Flag` are omitted from the labeled-field list and the first one is shown in the card header.
    /// Card layouts currently support at most one flag field.
    #[rerun(required)]
    pub field_order: Vec<rerun::blueprint::components::ColumnName>,
}
