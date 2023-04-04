/// Global options for the viewer.
#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppOptions {
    pub show_camera_axes_in_3d: bool,

    pub low_latency: f32,
    pub warn_latency: f32,

    /// Show milliseconds, RAM usage, etc.
    pub show_metrics: bool,

    /// Zoom factor, independent of OS points_per_pixel setting.
    ///
    /// At every frame we check the OS reported scaling (i.e. points_per_pixel)
    /// and apply this zoom factor to determine the actual points_per_pixel.
    /// This way, the zooming stays constant when switching between differently scaled screens.
    /// (Since this is serialized, even between sessions!)
    #[cfg(not(target_arch = "wasm32"))]
    pub zoom_factor: f32,

    /// Enable the experimental feature for space view screenshots.
    #[cfg(not(target_arch = "wasm32"))]
    pub experimental_space_view_screenshots: bool,

    /// Displays an overlay for debugging picking.
    pub show_picking_debug_overlay: bool,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            show_camera_axes_in_3d: true,

            low_latency: 0.100,
            warn_latency: 0.200,

            show_metrics: false,

            #[cfg(not(target_arch = "wasm32"))]
            zoom_factor: 1.0,

            #[cfg(not(target_arch = "wasm32"))]
            experimental_space_view_screenshots: false,

            show_picking_debug_overlay: false,
        }
    }
}
