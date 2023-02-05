/// Global options for the viewer.
#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppOptions {
    pub show_camera_axes_in_3d: bool,

    pub low_latency: f32,
    pub warn_latency: f32,

    /// Show milliseconds, RAM usage, etc.
    #[serde(skip)] // restore to the default for the current mode (dev vs debug)
    pub show_metrics: bool,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            show_camera_axes_in_3d: true,

            low_latency: 0.100,
            warn_latency: 0.200,

            show_metrics: cfg!(debug_assertions),
        }
    }
}
