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

    /// Use the legacy container blueprint storage for the space view.
    pub legacy_container_blueprint: bool,

    pub experimental_entity_filter_editor: bool,

    /// Enable the experimental support for the container addition workflow.
    pub experimental_additive_workflow: bool,

    /// Toggle primary caching for the 2D & 3D point cloud space views.
    pub experimental_primary_caching_point_clouds: bool,

    /// Toggle primary caching for the time series & text logs space views.
    pub experimental_primary_caching_series: bool,

    /// Displays an overlay for debugging picking.
    pub show_picking_debug_overlay: bool,

    /// Includes the blueprint in the timeline view.
    pub show_blueprint_in_timeline: bool,

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

            legacy_container_blueprint: false,

            experimental_entity_filter_editor: false,

            experimental_additive_workflow: cfg!(debug_assertions),

            experimental_primary_caching_point_clouds: true,
            experimental_primary_caching_series: true,

            show_picking_debug_overlay: false,

            show_blueprint_in_timeline: false,

            time_zone_for_timestamps: TimeZone::Utc,
        }
    }
}
