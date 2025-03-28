use std::path::PathBuf;

use re_log_types::TimestampFormat;
use re_video::decode::{DecodeHardwareAcceleration, DecodeSettings};

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

    /// Displays an overlay for debugging picking.
    pub show_picking_debug_overlay: bool,

    /// Inspect the blueprint timeline.
    pub inspect_blueprint_timeline: bool,

    /// Disable garbage collection of the blueprint.
    pub blueprint_gc: bool,

    /// What time zone to display timestamps in.
    #[serde(rename = "timestamp_format")]
    pub timestamp_format: TimestampFormat,

    /// Preferred method for video decoding on web.
    pub video_decoder_hw_acceleration: DecodeHardwareAcceleration,

    /// Override the path to the FFmpeg binary.
    ///
    /// If set, use `video_decoder_ffmpeg_path` as the path to the FFmpeg binary.
    /// Don't use this field directly, use [`AppOptions::video_decoder_settings`] instead.
    ///
    /// Implementation note: we avoid using `Option<PathBuf>` here to avoid loosing the user-defined
    /// path when disabling the override.
    #[allow(clippy::doc_markdown)]
    pub video_decoder_override_ffmpeg_path: bool,

    /// Custom path to the FFmpeg binary.
    ///
    /// Don't use this field directly, use [`AppOptions::video_decoder_settings`] instead.
    #[allow(clippy::doc_markdown)]
    pub video_decoder_ffmpeg_path: String,

    /// Mapbox API key (used to enable Mapbox-based map view backgrounds).
    ///
    /// Can also be set using the `RERUN_MAPBOX_ACCESS_TOKEN` environment variable.
    pub mapbox_access_token: String,

    pub enable_redap_browser: bool,

    /// Path to the directory suitable for storing cache data.
    ///
    /// By cache data, we mean data that is safe to be garbage collected by the OS. Defaults to
    /// to [`directories::ProjectDirs::cache_dir`].
    ///
    /// *NOTE*: subsystems making use of the cache directory should use a unique sub-directory name,
    /// see [`AppOptions::cache_subdirectory`].
    #[cfg(not(target_arch = "wasm32"))]
    pub cache_directory: Option<std::path::PathBuf>,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            low_latency: 0.100,
            warn_latency: 0.200,

            show_metrics: cfg!(debug_assertions),

            include_welcome_screen_button_in_recordings_panel: true,

            show_picking_debug_overlay: false,

            inspect_blueprint_timeline: false,

            blueprint_gc: true,

            timestamp_format: TimestampFormat::Utc,

            video_decoder_hw_acceleration: DecodeHardwareAcceleration::default(),
            video_decoder_override_ffmpeg_path: false,
            video_decoder_ffmpeg_path: String::new(),

            mapbox_access_token: String::new(),

            enable_redap_browser: false,

            #[cfg(not(target_arch = "wasm32"))]
            cache_directory: Self::default_cache_directory(),
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

    #[cfg(not(target_arch = "wasm32"))]
    pub fn cache_subdirectory(
        &self,
        sub_dir: impl AsRef<std::path::Path>,
    ) -> Option<std::path::PathBuf> {
        self.cache_directory
            .as_ref()
            .map(|cache_dir| cache_dir.join(sub_dir))
    }

    /// Default cache directory
    pub fn default_cache_directory() -> Option<std::path::PathBuf> {
        directories::ProjectDirs::from("io", "rerun", "Rerun")
            .map(|dirs| dirs.cache_dir().to_owned())
    }

    /// Get the video decoder settings.
    pub fn video_decoder_settings(&self) -> DecodeSettings {
        DecodeSettings {
            hw_acceleration: self.video_decoder_hw_acceleration,
            ffmpeg_path: self
                .video_decoder_override_ffmpeg_path
                .then(|| PathBuf::from(&self.video_decoder_ffmpeg_path)),
        }
    }
}
