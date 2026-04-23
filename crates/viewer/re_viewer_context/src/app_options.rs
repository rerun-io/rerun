use re_entity_db::FetchStage;
use re_log_types::TimestampFormat;
use re_memory::MemoryLimit;
use re_video::{DecodeHardwareAcceleration, DecodeSettings};

const MAPBOX_ACCESS_TOKEN_ENV_VAR: &str = "RERUN_MAPBOX_ACCESS_TOKEN";

/// Global options for the viewer.
#[derive(Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct AppOptions {
    /// Experimental feature flags.
    pub experimental: ExperimentalAppOptions,

    /// Warn if the e2e latency exceeds this value.
    pub warn_e2e_latency: f32,

    /// Show milliseconds, RAM usage, etc.
    pub show_metrics: bool,

    /// Show toasts for log messages?
    ///
    /// If false, you can still view them in the notifications panel.
    pub show_notification_toasts: bool,

    /// Include the "Welcome screen" application in the recordings panel?
    #[serde(alias = "include_welcome_screen_button_in_recordings_panel")]
    pub include_rerun_examples_button_in_recordings_panel: bool,

    /// Displays an overlay for debugging picking.
    pub show_picking_debug_overlay: bool,

    /// Inspect the blueprint timeline.
    pub inspect_blueprint_timeline: bool,

    /// Is garbage collection of the blueprint enabled?
    pub blueprint_gc: bool,

    /// What time zone to display timestamps in.
    #[serde(rename = "timestamp_format")]
    pub timestamp_format: TimestampFormat,

    /// Video decoding options.
    pub video: VideoOptions,

    /// Whether per-visualizer instance/element limits are enabled.
    ///
    /// Several visualizers (3D shapes, time series lines, etc.) cap the number of elements
    /// they process to avoid hangs. When disabled, those caps are removed entirely,
    /// which may cause the viewer to become unresponsive with very large data sets.
    pub visualizer_limits_enabled: bool,

    /// Mapbox API key (used to enable Mapbox-based map view backgrounds).
    ///
    /// Can also be set using the `RERUN_MAPBOX_ACCESS_TOKEN` environment variable.
    pub mapbox_access_token: String,

    /// When the total process RAM reaches this limit, we GC old data.
    pub memory_limit: MemoryLimit,

    /// Only prefetch chunks up to (and including) this stage.
    ///
    /// Useful for debugging and for users who want to limit how aggressively
    /// we prefetch data ahead of what is strictly needed.
    pub max_fetch_stage: FetchStage,

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
            experimental: Default::default(),

            warn_e2e_latency: 1.0,

            show_metrics: cfg!(debug_assertions),

            show_notification_toasts: true,

            include_rerun_examples_button_in_recordings_panel: true,

            show_picking_debug_overlay: false,

            inspect_blueprint_timeline: false,

            blueprint_gc: true,

            timestamp_format: TimestampFormat::default(),

            video: Default::default(),

            visualizer_limits_enabled: true,

            mapbox_access_token: String::new(),

            memory_limit: if cfg!(target_arch = "wasm32") {
                // On wasm32 we only have 4GB of memory to play around with.
                re_memory::MemoryLimit::from_bytes(2_500_000_000)
            } else {
                MemoryLimit::from_fraction_of_total(0.75)
            },

            max_fetch_stage: FetchStage::default(),

            #[cfg(not(target_arch = "wasm32"))]
            cache_directory: Self::default_cache_directory(),
        }
    }
}

impl AppOptions {
    pub fn test() -> Self {
        Self {
            memory_limit: MemoryLimit::UNLIMITED,
            show_metrics: false, // flaky in snapshot tests
            ..Default::default()
        }
    }

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
            hw_acceleration: self.video.hw_acceleration,

            #[cfg(not(target_arch = "wasm32"))]
            ffmpeg_path: self
                .video
                .override_ffmpeg_path
                .then(|| std::path::PathBuf::from(&self.video.ffmpeg_path)),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct VideoOptions {
    /// Preferred method for video decoding on web.
    pub hw_acceleration: DecodeHardwareAcceleration,

    /// Override the path to the FFmpeg binary.
    ///
    /// If set, use `video_decoder_ffmpeg_path` as the path to the FFmpeg binary.
    /// Don't use this field directly, use [`AppOptions::video_decoder_settings`] instead.
    ///
    /// Implementation note: we avoid using `Option<PathBuf>` here to avoid losing the user-defined
    /// path when disabling the override.
    #[expect(clippy::doc_markdown)]
    pub override_ffmpeg_path: bool,

    /// Custom path to the FFmpeg binary.
    ///
    /// Don't use this field directly, use [`AppOptions::video_decoder_settings`] instead.
    #[expect(clippy::doc_markdown)]
    pub ffmpeg_path: String,
}

#[derive(Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ExperimentalAppOptions {
    /// Enable the experimental Status view.
    pub enable_status_view: bool,

    /// Enable grid view mode for data tables.
    ///
    /// When enabled, a list/grid toggle appears in the table title bar,
    /// allowing users to switch between the traditional table and a card-based grid layout.
    pub table_grid_view: bool,
}
