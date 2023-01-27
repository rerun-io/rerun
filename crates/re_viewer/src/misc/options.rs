/// Global options for the viewer.
#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Options {
    pub show_camera_axes_in_3d: bool,

    pub low_latency: f32,
    pub warn_latency: f32,

    pub debug: DebugOptions,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            show_camera_axes_in_3d: true,

            low_latency: 0.100,
            warn_latency: 0.200,

            debug: Default::default(),
        }
    }
}

impl Options {
    pub fn show_dev_controls(&self) -> bool {
        cfg!(debug_assertions) && !self.debug.extra_clean_ui
    }

    pub fn spaceview_hover_controls(&self) -> bool {
        !self.debug.extra_clean_ui
    }
}

#[derive(Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct DebugOptions {
    /// Clean up the UI, e.g. for screen recordings.
    pub extra_clean_ui: bool,
}
