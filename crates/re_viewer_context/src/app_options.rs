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

    /// Toggle query clamping for the plot visualizers.
    pub experimental_plot_query_clamping: bool,

    /// Toggle explicit visualizer selection for all views.
    pub experimental_visualizer_selection: bool,

    /// Displays an overlay for debugging picking.
    pub show_picking_debug_overlay: bool,

    /// Inspect the blueprint timeline.
    pub inspect_blueprint_timeline: bool,

    /// Disable garbage collection of the blueprint.
    pub blueprint_gc: bool,

    /// What time zone to display timestamps in.
    #[serde(rename = "time_zone_for_timestamps")]
    pub time_zone: TimeZone,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            low_latency: 0.100,
            warn_latency: 0.200,

            show_metrics: cfg!(debug_assertions),

            #[cfg(not(target_arch = "wasm32"))]
            experimental_space_view_screenshots: false,

            experimental_dataframe_space_view: false,

            experimental_entity_filter_editor: false,

            experimental_plot_query_clamping: false,

            experimental_visualizer_selection: false,

            show_picking_debug_overlay: false,

            inspect_blueprint_timeline: false,

            blueprint_gc: true,

            time_zone: TimeZone::Utc,
        }
    }
}
