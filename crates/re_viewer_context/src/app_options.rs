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

    /// Enable experimental support for new container blueprints
    pub experimental_container_blueprints: bool,

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

            experimental_container_blueprints: cfg!(debug_assertions),

            show_picking_debug_overlay: false,

            show_blueprint_in_timeline: false,

            time_zone_for_timestamps: TimeZone::Utc,
        }
    }
}
