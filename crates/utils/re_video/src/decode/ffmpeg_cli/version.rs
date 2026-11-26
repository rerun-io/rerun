use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::task::Poll;

use re_mutex::Mutex;

// FFmpeg 5.1 "Riemann" is from 2022-07-22.
// It's simply the oldest I tested manually as of writing. We might be able to go lower.
// However, we also know that FFmpeg 4.4 is already no longer working.
pub const FFMPEG_MINIMUM_VERSION_MAJOR: u32 = 5;
pub const FFMPEG_MINIMUM_VERSION_MINOR: u32 = 1;

pub type FfmpegVersionResult = Result<FFmpegVersion, FFmpegVersionParseError>;
type SharedVersionResult = Arc<OnceLock<FfmpegVersionResult>>;

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
    #[error("Failed to read FFmpeg executable metadata: {0}")]
    RetrieveFileModificationTime(String),

    #[error("Failed to run FFmpeg: {0}")]
    RunFFmpeg(String),

    #[error("FFmpeg not found at {0}")]
    FFmpegNotFound(String),

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
    /// Internally caches the result per executable path.
    pub fn for_executable_poll(path: Option<&std::path::Path>) -> Poll<FfmpegVersionResult> {
        re_tracing::profile_function!();

        check_executable_path(path)?;
        let version = VersionCache::global(|cache| cache.version(path));
        match version.get() {
            Some(result) => Poll::Ready(result.clone()),
            None => Poll::Pending,
        }
    }

    /// Like [`Self::for_executable_poll`], but blocks until the version is ready.
    ///
    /// WARNING: this can block for SEVEN SECONDS on Mac, but usually only the first time
    /// after a reboot. NEVER call this on the GUI thread!
    pub fn for_executable_blocking(path: Option<&std::path::Path>) -> FfmpegVersionResult {
        re_tracing::profile_function!();

        check_executable_path(path)?;

        let version = VersionCache::global(|cache| cache.version(path));

        re_tracing::profile_wait!("ffmpeg-version");
        version.wait().clone()
    }

    /// Returns true if this version is compatible with Rerun's minimum requirements.
    pub fn is_compatible(&self) -> bool {
        self.major > FFMPEG_MINIMUM_VERSION_MAJOR
            || (self.major == FFMPEG_MINIMUM_VERSION_MAJOR
                && self.minor >= FFMPEG_MINIMUM_VERSION_MINOR)
    }

    /// Keeps the version of the executable at `path` pending until the returned cell is set.
    ///
    /// Set the result before leaving the test so any waiters can finish.
    #[cfg(test)]
    pub fn pending_for_test(path: &std::path::Path) -> SharedVersionResult {
        let pending = Arc::new(OnceLock::new());
        VersionCache::global(|VersionCache(cache)| {
            cache.insert(path.to_path_buf(), pending.clone());
        });
        pending
    }
}

fn check_executable_path(path: Option<&std::path::Path>) -> Result<(), FFmpegVersionParseError> {
    if let Some(path) = path {
        path.metadata().map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                FFmpegVersionParseError::FFmpegNotFound(path.display().to_string())
            } else {
                FFmpegVersionParseError::RetrieveFileModificationTime(err.to_string())
            }
        })?;
    }
    Ok(())
}

/// The `Arc` lets callers wait on a result without holding the cache lock.
#[derive(Default)]
struct VersionCache(HashMap<PathBuf, SharedVersionResult>);

impl VersionCache {
    fn global<R>(f: impl FnOnce(&mut Self) -> R) -> R {
        static CACHE: std::sync::LazyLock<Mutex<VersionCache>> =
            std::sync::LazyLock::new(|| Mutex::new(VersionCache::default()));
        f(&mut CACHE.lock())
    }

    fn version(&mut self, path: Option<&std::path::Path>) -> SharedVersionResult {
        let Self(cache) = self;

        let cache_key = path
            .unwrap_or_else(|| std::path::Path::new("ffmpeg"))
            .to_path_buf();

        match cache.entry(cache_key) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.get().clone(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let version = Arc::new(OnceLock::new());
                let path = path.map(|path| path.to_path_buf());
                std::thread::Builder::new()
                    .name("ffmpeg_version".to_owned())
                    .spawn({
                        let version = version.clone();
                        move || {
                            version.set(ffmpeg_version(path.as_ref())).ok();
                        }
                    })
                    .expect("Failed to spawn thread");
                entry.insert(version.clone());
                version
            }
        }
    }
}

fn ffmpeg_version(
    path: Option<&std::path::PathBuf>,
) -> Result<FFmpegVersion, FFmpegVersionParseError> {
    re_tracing::profile_function!();

    // Don't use sidecar's ffmpeg_version_with_path/ffmpeg_version directly since the error message for
    // file not found isn't great and we want this for display in the UI.

    let path = path.cloned().unwrap_or_else(|| {
        re_tracing::profile_scope!("ffmpeg_path");
        ffmpeg_sidecar::paths::ffmpeg_path()
    });
    let mut cmd = match std::process::Command::new(&path)
        .arg("-version")
        .stdout(std::process::Stdio::piped()) // not stderr when calling `-version`
        .spawn()
    {
        Ok(cmd) => cmd,
        Err(err) => {
            return Err(if err.kind() == std::io::ErrorKind::NotFound {
                FFmpegVersionParseError::FFmpegNotFound(path.display().to_string())
            } else {
                FFmpegVersionParseError::RunFFmpeg(err.to_string())
            });
        }
    };
    let Some(stdout) = cmd.stdout.take() else {
        return Err(FFmpegVersionParseError::RunFFmpeg(
            "No standard output channel".to_owned(),
        ));
    };

    let mut parser = ffmpeg_sidecar::log_parser::FfmpegLogParser::new(stdout);
    let mut raw_version = None;
    while let Ok(event) = parser.parse_next_event() {
        match event {
            ffmpeg_sidecar::event::FfmpegEvent::ParsedVersion(v) => raw_version = Some(v.version),
            ffmpeg_sidecar::event::FfmpegEvent::LogEOF => break,
            _ => {}
        }
    }
    cmd.wait().ok(); // Don't care if it fails here.

    let raw_version = raw_version.ok_or_else(|| FFmpegVersionParseError::ParseVersion {
        raw_version: "No version found".to_owned(),
    })?;

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
    use std::task::Poll;
    use std::time::Duration;

    use super::FFmpegVersion;

    /// Regression test for <https://github.com/rerun-io/rerun/issues/10789>.
    #[test]
    fn poll_does_not_wait_for_blocking_caller() {
        // The cache is global, so use an existing path no other test uses.
        let path = std::env::current_exe().unwrap();
        let pending = FFmpegVersion::pending_for_test(&path);

        let blocking = std::thread::Builder::new()
            .name("test_blocking".to_owned())
            .spawn({
                let path = path.clone();
                move || FFmpegVersion::for_executable_blocking(Some(&path))
            })
            .unwrap();
        // Give the blocking caller time to start waiting.
        std::thread::sleep(Duration::from_millis(100));

        // Poll on another thread so that a regression fails the test instead of hanging it.
        let (poll_tx, poll_rx) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("test_poll".to_owned())
            .spawn({
                let path = path.clone();
                move || poll_tx.send(FFmpegVersion::for_executable_poll(Some(&path)).is_pending())
            })
            .unwrap();
        let poll_was_pending = poll_rx.recv_timeout(Duration::from_secs(5));

        let version = FFmpegVersion::parse("7.1").unwrap();
        pending
            .set(Ok(version.clone()))
            .expect("cached FFmpeg version should still be pending");

        assert_eq!(
            poll_was_pending,
            Ok(true),
            "for_executable_poll waited for for_executable_blocking"
        );
        assert_eq!(blocking.join().unwrap().unwrap(), version);
        assert_eq!(
            FFmpegVersion::for_executable_poll(Some(&path)).map(Result::unwrap),
            Poll::Ready(version)
        );
    }

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
