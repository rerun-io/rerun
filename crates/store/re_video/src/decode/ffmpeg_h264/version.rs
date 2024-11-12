use std::{collections::HashMap, path::PathBuf};

use once_cell::sync::Lazy;
use parking_lot::Mutex;

// FFmpeg 5.1 "Riemann" is from 2022-07-22.
// It's simply the oldest I tested manually as of writing. We might be able to go lower.
// However, we also know that FFmpeg 4.4 is already no longer working.
pub const FFMPEG_MINIMUM_VERSION_MAJOR: u32 = 5;
pub const FFMPEG_MINIMUM_VERSION_MINOR: u32 = 1;

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
    pub fn for_executable(path: Option<&std::path::Path>) -> Result<Self, FFmpegVersionParseError> {
        type VersionMap = HashMap<
            PathBuf,
            (
                Option<std::time::SystemTime>,
                Result<FFmpegVersion, FFmpegVersionParseError>,
            ),
        >;
        static CACHE: Lazy<Mutex<VersionMap>> = Lazy::new(|| Mutex::new(HashMap::new()));

        re_tracing::profile_function!();

        // Retrieve file modification time first.
        let modification_time = if let Some(path) = path {
            path.metadata()
                .map_err(|err| {
                    FFmpegVersionParseError::RetrieveFileModificationTime(err.to_string())
                })?
                .modified()
                .ok()
        } else {
            None
        };

        // Check first if we already have the version cached.
        let mut cache = CACHE.lock();
        let cache_key = path.unwrap_or(std::path::Path::new("ffmpeg"));
        if let Some(cached) = cache.get(cache_key) {
            if modification_time == cached.0 {
                return cached.1.clone();
            }
        }

        // Run FFmpeg (or whatever was passed to us) to get the version.
        let raw_version = if let Some(path) = path {
            ffmpeg_sidecar::version::ffmpeg_version_with_path(path)
        } else {
            ffmpeg_sidecar::version::ffmpeg_version()
        }
        .map_err(|err| FFmpegVersionParseError::RunFFmpeg(err.to_string()))?;

        let version = if let Some(version) = Self::parse(&raw_version) {
            Ok(version)
        } else {
            Err(FFmpegVersionParseError::ParseVersion {
                raw_version: raw_version.clone(),
            })
        };

        cache.insert(
            cache_key.to_path_buf(),
            (modification_time, version.clone()),
        );

        version
    }

    /// Returns true if this version is compatible with Rerun's minimum requirements.
    pub fn is_compatible(&self) -> bool {
        self.major > FFMPEG_MINIMUM_VERSION_MAJOR
            || (self.major == FFMPEG_MINIMUM_VERSION_MAJOR
                && self.minor >= FFMPEG_MINIMUM_VERSION_MINOR)
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
