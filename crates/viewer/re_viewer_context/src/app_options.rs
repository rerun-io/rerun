use re_log_types::TimeZone;

const MAPBOX_ACCESS_TOKEN_ENV_VAR: &str = "RERUN_MAPBOX_ACCESS_TOKEN";

/// Global options for the viewer.
#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppOptions {
    pub low_latency: f32,
    pub warn_latency: f32,

    /// Show milliseconds, RAM usage, etc.
    pub show_metrics: bool,

    /// Include the "Welcome screen" application in the recordings panel?
    pub include_welcome_screen_button_in_recordings_panel: bool,

    /// Enable the experimental feature for space view screenshots.
    #[cfg(not(target_arch = "wasm32"))]
    pub experimental_space_view_screenshots: bool,

    /// Displays an overlay for debugging picking.
    pub show_picking_debug_overlay: bool,

    /// Inspect the blueprint timeline.
    pub inspect_blueprint_timeline: bool,

    /// Disable garbage collection of the blueprint.
    pub blueprint_gc: bool,

    /// What time zone to display timestamps in.
    #[serde(rename = "time_zone_for_timestamps")]
    pub time_zone: TimeZone,

    /// Hardware acceleration settings for video decoding.
    pub video_decoder_hw_acceleration: re_video::decode::DecodeHardwareAcceleration,

    /// Mapbox API key (used to enable Mapbox-based map view backgrounds).
    ///
    /// Can also be set using the `RERUN_MAPBOX_ACCESS_TOKEN` environment variable.
    pub mapbox_access_token: String,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            low_latency: 0.100,
            warn_latency: 0.200,

            show_metrics: cfg!(debug_assertions),

            include_welcome_screen_button_in_recordings_panel: true,

            #[cfg(not(target_arch = "wasm32"))]
            experimental_space_view_screenshots: false,

            show_picking_debug_overlay: false,

            inspect_blueprint_timeline: false,

            blueprint_gc: true,

            time_zone: TimeZone::Utc,

            video_decoder_hw_acceleration: Default::default(),

            mapbox_access_token: String::new(),
        }
    }
}

impl AppOptions {
    pub fn mapbox_access_token(&self) -> Option<String> {
        if self.mapbox_access_token.is_empty() {
            std::env::var(MAPBOX_ACCESS_TOKEN_ENV_VAR).ok()
        } else {
            Some(self.mapbox_access_token.clone())
        }
    }
}
