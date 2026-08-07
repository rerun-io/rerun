// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The query for the dataframe view.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct DataframeQuery {
    /// The timeline for this query.
    ///
    /// If unset, the timeline currently active on the time panel is used.
    #[rerun(optional)]
    pub timeline: Option<rerun::blueprint::components::TimelineName>,

    /// If provided, only rows whose timestamp is within this range will be shown.
    ///
    /// Note: will be unset as soon as `timeline` is changed.
    #[rerun(optional)]
    pub filter_by_range: Option<rerun::blueprint::components::FilterByRange>,

    /// If provided, only show rows which contains a logged event for the specified component.
    #[rerun(optional)]
    pub filter_is_not_null: Option<rerun::blueprint::components::FilterIsNotNull>,

    /// Should empty cells be filled with latest-at queries?
    #[rerun(optional)]
    pub apply_latest_at: Option<rerun::blueprint::components::ApplyLatestAt>,

    /// Selected columns. If unset, only the active timeline and all component columns are selected.
    #[rerun(optional)]
    pub select: Option<rerun::blueprint::components::SelectedColumns>,

    /// The order of entity path column groups. If unset, the default order is used.
    ///
    /// This affects the order of component columns, which are always grouped by entity path. Timeline columns always
    /// come first. Entities not listed here are appended at the end in default order.
    ///
    /// If `entity_order` contains any entity path that is not included in the view, they are ignored.
    #[rerun(optional)]
    pub entity_order: Option<rerun::blueprint::components::ColumnOrder>,

    /// Whether to auto-scroll to track the time cursor.
    ///
    /// When enabled and the view's timeline matches the time panel's active timeline,
    /// the view will scroll to keep the row at or before the current time cursor visible.
    #[rerun(optional)]
    pub auto_scroll: Option<rerun::blueprint::components::AutoScroll>,
}
