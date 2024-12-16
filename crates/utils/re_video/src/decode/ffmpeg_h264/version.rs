use std::{collections::HashMap, path::PathBuf, task::Poll};

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use poll_promise::Promise;

// FFmpeg 5.1 "Riemann" is from 2022-07-22.
// It's simply the oldest I tested manually as of writing. We might be able to go lower.
// However, we also know that FFmpeg 4.4 is already no longer working.
pub const FFMPEG_MINIMUM_VERSION_MAJOR: u32 = 5;
pub const FFMPEG_MINIMUM_VERSION_MINOR: u32 = 1;

pub type FfmpegVersionResult = Result<FFmpegVersion, FFmpegVersionParseError>;

/// A successfully parsed `FFmpeg` version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FFmpegVersion {
    major: u32,
    minor: u32,
    raw_version: String,
}

impl std::fmt::Display for FFmpegVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Alternative with more information:
        //write!(f, "{}.{} ({})", self.major, self.minor, self.raw_version)
        // The drawback of that is that it can look repetitive for trivial versions.
        // So let's show just the raw version instead since that's the more important information.
        self.raw_version.fmt(f)
    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum FFmpegVersionParseError {
    #[error("Failed to retrieve file modification time of FFmpeg executable: {0}")]
    RetrieveFileModificationTime(String),

    #[error("Failed to determine FFmpeg version: {0}")]
    RunFFmpeg(String),

    #[error("Failed to parse FFmpeg version: {raw_version}")]
    ParseVersion { raw_version: String },
}

impl FFmpegVersion {
    pub fn parse(raw_version: &str) -> Option<Self> {
        // Version strings can get pretty wild!
        // E.g.
        // * choco installed ffmpeg on Windows gives me "7.1-essentials_build-www.gyan.dev".
        // * Linux builds may come with `n7.0.2`
        // Seems to be easiest to just strip out any non-digit first.
        let stripped_version = raw_version
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>();
        let mut version_parts = stripped_version.split('.');

        // Major version is a strict requirement.
        let major = version_parts
            .next()
            .and_then(|part| part.parse::<u32>().ok())?;

        // Minor version is optional.
        let minor = version_parts
            .next()
            .and_then(|part| part.parse::<u32>().ok())
            .unwrap_or(0);

        Some(Self {
            major,
            minor,
            raw_version: raw_version.to_owned(),
        })
    }

    /// Try to parse the `FFmpeg` version for a given `FFmpeg` executable.
    ///
    /// If none is passed for the path, it uses `ffmpeg` from PATH.
    ///
    /// Error indicates issues running `FFmpeg`. Ok(None) indicates that we weren't able to parse the
    /// version string. Since version strings can get pretty wild, we don't want to fail in this case.
    ///
    /// Internally caches the result per path together with its modification time to re-run/parse the version only if the file has changed.
    pub fn for_executable_poll(path: Option<&std::path::Path>) -> Poll<FfmpegVersionResult> {
        re_tracing::profile_function!();

        let modification_time = file_modification_time(path)?;
        VersionCache::global(|cache| {
            cache
                .version(path, modification_time)
                .poll()
                .map(|r| r.clone())
        })
    }

    /// Like [`Self::for_executable_poll`], but blocks until the version is ready.
    ///
    /// WARNING: this blocks for half a second on Mac the first time this is called with a given path, maybe more on other platforms.
    pub fn for_executable_blocking(path: Option<&std::path::Path>) -> FfmpegVersionResult {
        re_tracing::profile_function!();

        let modification_time = file_modification_time(path)?;
        VersionCache::global(|cache| {
            cache
                .version(path, modification_time)
                .block_until_ready()
                .clone()
        })
    }

    /// Returns true if this version is compatible with Rerun's minimum requirements.
    pub fn is_compatible(&self) -> bool {
        self.major > FFMPEG_MINIMUM_VERSION_MAJOR
            || (self.major == FFMPEG_MINIMUM_VERSION_MAJOR
                && self.minor >= FFMPEG_MINIMUM_VERSION_MINOR)
    }
}

fn file_modification_time(
    path: Option<&std::path::Path>,
) -> Result<Option<std::time::SystemTime>, FFmpegVersionParseError> {
    Ok(if let Some(path) = path {
        path.metadata()
            .map_err(|err| FFmpegVersionParseError::RetrieveFileModificationTime(err.to_string()))?
            .modified()
            .ok()
    } else {
        None
    })
}

#[derive(Default)]
struct VersionCache(
    HashMap<PathBuf, (Option<std::time::SystemTime>, Promise<FfmpegVersionResult>)>,
);

impl VersionCache {
    fn global<R>(f: impl FnOnce(&mut Self) -> R) -> R {
        static CACHE: Lazy<Mutex<VersionCache>> = Lazy::new(|| Mutex::new(VersionCache::default()));
        f(&mut CACHE.lock())
    }

    fn version(
        &mut self,
        path: Option<&std::path::Path>,
        modification_time: Option<std::time::SystemTime>,
    ) -> &Promise<FfmpegVersionResult> {
        let Self(cache) = self;

        let cache_key = path.unwrap_or(std::path::Path::new("ffmpeg")).to_path_buf();

        match cache.entry(cache_key) {
            std::collections::hash_map::Entry::Occupied(entry) => &entry.into_mut().1,
            std::collections::hash_map::Entry::Vacant(entry) => {
                let path = path.map(|path| path.to_path_buf());
                let version =
                    Promise::spawn_thread("ffmpeg_version", move || ffmpeg_version(path.as_ref()));
                &entry.insert((modification_time, version)).1
            }
        }
    }
}

fn ffmpeg_version(
    path: Option<&std::path::PathBuf>,
) -> Result<FFmpegVersion, FFmpegVersionParseError> {
    re_tracing::profile_function!("ffmpeg_version_with_path");

    let raw_version = if let Some(path) = path {
        ffmpeg_sidecar::version::ffmpeg_version_with_path(path)
    } else {
        ffmpeg_sidecar::version::ffmpeg_version()
    }
    .map_err(|err| FFmpegVersionParseError::RunFFmpeg(err.to_string()))?;

    if let Some(version) = FFmpegVersion::parse(&raw_version) {
        Ok(version)
    } else {
        Err(FFmpegVersionParseError::ParseVersion {
            raw_version: raw_version.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::decode::ffmpeg_h264::FFmpegVersion;

    #[test]
    fn test_parse_ffmpeg_version() {
        // Real world examples:
        assert_eq!(
            FFmpegVersion::parse("7.1"),
            Some(FFmpegVersion {
                major: 7,
                minor: 1,
                raw_version: "7.1".to_owned(),
            })
        );
        assert_eq!(
            FFmpegVersion::parse("4.4.5"),
            Some(FFmpegVersion {
                major: 4,
                minor: 4,
                raw_version: "4.4.5".to_owned(),
            })
        );
        assert_eq!(
            FFmpegVersion::parse("7.1.2-essentials_build-www.gyan.dev"),
            Some(FFmpegVersion {
                major: 7,
                minor: 1,
                raw_version: "7.1.2-essentials_build-www.gyan.dev".to_owned(),
            })
        );
        assert_eq!(
            FFmpegVersion::parse("n7.0.2"),
            Some(FFmpegVersion {
                major: 7,
                minor: 0,
                raw_version: "n7.0.2".to_owned(),
            })
        );

        // Made up stuff:
        assert_eq!(
            FFmpegVersion::parse("123"),
            Some(FFmpegVersion {
                major: 123,
                minor: 0,
                raw_version: "123".to_owned(),
            })
        );
        assert_eq!(
            FFmpegVersion::parse("lol321wut.23"),
            Some(FFmpegVersion {
                major: 321,
                minor: 23,
                raw_version: "lol321wut.23".to_owned(),
            })
        );
    }
}
