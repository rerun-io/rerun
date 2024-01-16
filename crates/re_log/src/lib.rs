//! Text logging (nothing to do with rerun logging) for use in rerun libraries.
//!
//! Provides helpers for adding multiple loggers,
//! and for setting up logging on native and on web.
//!
//! * `trace`: spammy things
//! * `debug`: things that might be useful when debugging
//! * `info`: things that we want to show to users
//! * `warn`: problems that we can recover from
//! * `error`: problems that lead to loss of functionality or data
//!
//! The `warn_once` etc macros are for when you want to suppress repeated
//! logging of the exact same message.

mod channel_logger;
mod result_extensions;

#[cfg(feature = "setup")]
mod multi_logger;

#[cfg(feature = "setup")]
mod setup;

#[cfg(all(feature = "setup", target_arch = "wasm32"))]
mod web_logger;

pub use log::{Level, LevelFilter};

pub use result_extensions::ResultExt;

// The tracing macros support more syntax features than the log, that's why we use them:
pub use tracing::{debug, error, info, trace, warn};

// The `re_log::info_once!(…)` etc are nice helpers, but the `log-once` crate is a bit lacking.
// In the future we should implement our own macros to de-duplicate based on the callsite,
// similar to how the log console in a browser will automatically suppress duplicates.
pub use log_once::{debug_once, error_once, info_once, log_once, trace_once, warn_once};

pub use channel_logger::*;

#[cfg(feature = "setup")]
pub use multi_logger::{add_boxed_logger, add_logger, MultiLoggerNotSetupError};

#[cfg(feature = "setup")]
pub use setup::*;

/// Re-exports of other crates.
pub mod external {
    pub use log;
}

/// Never log anything less serious than a `ERROR` from these crates.
const CRATES_AT_ERROR_LEVEL: &[&str] = &[
    // Waiting for https://github.com/etemesi254/zune-image/issues/131 to be released
    "zune_jpeg",
    // silence rustls in release mode: https://github.com/rerun-io/rerun/issues/3104
    #[cfg(not(debug_assertions))]
    "rustls",
];

/// Never log anything less serious than a `WARN` from these crates.
const CRATES_AT_WARN_LEVEL: &[&str] = &[
    // wgpu crates spam a lot on info level, which is really annoying
    // TODO(emilk): remove once https://github.com/gfx-rs/wgpu/issues/3206 is fixed
    "naga",
    "wgpu_core",
    "wgpu_hal",
];

/// Never log anything less serious than a `INFO` from these crates.
const CRATES_AT_INFO_LEVEL: &[&str] = &[
    // These are quite spammy on debug, drowning out what we care about:
    "h2",
    "hyper",
    "ureq",
    // only let rustls run in debug mode: https://github.com/rerun-io/rerun/issues/3104
    #[cfg(debug_assertions)]
    "rustls",
];

/// Get `RUST_LOG` environment variable or `info`, if not set.
///
/// Also sets some other log levels on crates that are too loud.
#[cfg(not(target_arch = "wasm32"))]
pub fn default_log_filter() -> String {
    let mut rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            "debug".to_owned()
        } else {
            "info".to_owned()
        }
    });

    for crate_name in crate::CRATES_AT_ERROR_LEVEL {
        if !rust_log.contains(&format!("{crate_name}=")) {
            rust_log += &format!(",{crate_name}=error");
        }
    }
    for crate_name in crate::CRATES_AT_WARN_LEVEL {
        if !rust_log.contains(&format!("{crate_name}=")) {
            rust_log += &format!(",{crate_name}=warn");
        }
    }
    for crate_name in crate::CRATES_AT_INFO_LEVEL {
        if !rust_log.contains(&format!("{crate_name}=")) {
            rust_log += &format!(",{crate_name}=info");
        }
    }

    rust_log
}

/// Should we log this message given the filter?
fn is_log_enabled(filter: log::LevelFilter, metadata: &log::Metadata<'_>) -> bool {
    if CRATES_AT_ERROR_LEVEL
        .iter()
        .any(|crate_name| metadata.target().starts_with(crate_name))
    {
        return metadata.level() <= log::LevelFilter::Error;
    }

    if CRATES_AT_WARN_LEVEL
        .iter()
        .any(|crate_name| metadata.target().starts_with(crate_name))
    {
        return metadata.level() <= log::LevelFilter::Warn;
    }

    if CRATES_AT_INFO_LEVEL
        .iter()
        .any(|crate_name| metadata.target().starts_with(crate_name))
    {
        return metadata.level() <= log::LevelFilter::Info;
    }

    metadata.level() <= filter
}

/// Shorten a path to a Rust source file.
///
/// Example input:
/// * `/Users/emilk/.cargo/registry/src/github.com-1ecc6299db9ec823/tokio-1.24.1/src/runtime/runtime.rs`
/// * `crates/rerun/src/main.rs`
/// * `/rustc/d5a82bbd26e1ad8b7401f6a718a9c57c96905483/library/core/src/ops/function.rs`
///
/// Example output:
/// * `tokio-1.24.1/src/runtime/runtime.rs`
/// * `rerun/src/main.rs`
/// * `core/src/ops/function.rs`
#[allow(dead_code)] // only used on web and in tests
fn shorten_file_path(file_path: &str) -> &str {
    if let Some(i) = file_path.rfind("/src/") {
        if let Some(prev_slash) = file_path[..i].rfind('/') {
            &file_path[prev_slash + 1..]
        } else {
            file_path
        }
    } else {
        file_path
    }
}

#[test]
fn test_shorten_file_path() {
    for (before, after) in [
        ("/Users/emilk/.cargo/registry/src/github.com-1ecc6299db9ec823/tokio-1.24.1/src/runtime/runtime.rs", "tokio-1.24.1/src/runtime/runtime.rs"),
        ("crates/rerun/src/main.rs", "rerun/src/main.rs"),
        ("/rustc/d5a82bbd26e1ad8b7401f6a718a9c57c96905483/library/core/src/ops/function.rs", "core/src/ops/function.rs"),
        ("/weird/path/file.rs", "/weird/path/file.rs"),
        ]
    {
        assert_eq!(shorten_file_path(before), after);
    }
}
