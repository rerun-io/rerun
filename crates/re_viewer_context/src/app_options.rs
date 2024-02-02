use re_log_types::TimeZone;

/// Global options for the viewer.
#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppOptions {
    pub low_latency: f32,
    pub warn_latency: f32,

    /// Show milliseconds, RAM usage, etc.
    pub show_metrics: bool,

    /// Enable the experimental feature for space view screenshots.
    #[cfg(not(target_arch = "wasm32"))]
    pub experimental_space_view_screenshots: bool,

    /// Enable experimental dataframe space views.
    pub experimental_dataframe_space_view: bool,

    pub experimental_entity_filter_editor: bool,

    /// Enable the experimental support for the container addition workflow.
    pub experimental_additive_workflow: bool,

    /// Toggle primary caching for latest-at queries.
    ///
    /// Applies to the 2D/3D point cloud, 2D/3D box, text log and time series space views.
    pub experimental_primary_caching_latest_at: bool,

    /// Toggle primary caching for range queries.
    ///
    /// Applies to the 2D/3D point cloud, 2D/3D box, text log and time series space views.
    pub experimental_primary_caching_range: bool,

    /// Toggle query clamping for the plot visualizers.
    pub experimental_plot_query_clamping: bool,

    /// Displays an overlay for debugging picking.
    pub show_picking_debug_overlay: bool,

    /// Inspect the blueprint timeline.
    pub inspect_blueprint_timeline: bool,

    /// Disable garbage collection of the blueprint.
    pub blueprint_gc: bool,

    /// What time zone to display timestamps in.
    pub time_zone_for_timestamps: TimeZone,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            low_latency: 0.100,
            warn_latency: 0.200,

            show_metrics: false,

            #[cfg(not(target_arch = "wasm32"))]
            experimental_space_view_screenshots: false,

            experimental_dataframe_space_view: false,

            experimental_entity_filter_editor: false,

            experimental_additive_workflow: cfg!(debug_assertions),

            experimental_primary_caching_latest_at: true,
            experimental_primary_caching_range: true,

            experimental_plot_query_clamping: false,

            show_picking_debug_overlay: false,

            inspect_blueprint_timeline: false,

            blueprint_gc: true,

            time_zone_for_timestamps: TimeZone::Utc,
        }
    }
}
