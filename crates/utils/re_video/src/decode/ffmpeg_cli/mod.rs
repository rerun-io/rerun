mod ffmpeg;
mod version;

pub use ffmpeg::{Error, FFmpegCliDecoder};
pub use version::{
    FFMPEG_MINIMUM_VERSION_MAJOR, FFMPEG_MINIMUM_VERSION_MINOR, FFmpegVersion,
    FFmpegVersionParseError,
};

/// Download URL for the latest version of `FFmpeg` on the current platform.
/// None if the platform is not supported.
// TODO(andreas): as of writing, ffmpeg-sidecar doesn't define a download URL for linux arm.
pub fn ffmpeg_download_url() -> Option<&'static str> {
    ffmpeg_sidecar::download::ffmpeg_download_url().ok()
}
