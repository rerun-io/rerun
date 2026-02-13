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
//!
//! In the viewer these logs, if >= info, become notifications. See
//! `re_ui::notifications` for more information.

mod channel_logger;
mod debug_assert;
mod result_extensions;

#[cfg(feature = "setup")]
mod multi_logger;

#[cfg(feature = "setup")]
mod setup;

#[cfg(all(feature = "setup", target_arch = "wasm32"))]
mod web_logger;

pub use channel_logger::*;
pub use log::{Level, LevelFilter};
// The `re_log::info_once!(…)` etc are nice helpers, but the `log-once` crate is a bit lacking.
// In the future we should implement our own macros to de-duplicate based on the callsite,
// similar to how the log console in a browser will automatically suppress duplicates.
pub use log_once::{debug_once, error_once, info_once, log_once, trace_once, warn_once};
#[cfg(feature = "setup")]
pub use multi_logger::{MultiLoggerNotSetupError, add_boxed_logger, add_logger};
pub use result_extensions::ResultExt;
#[cfg(all(feature = "setup", not(target_arch = "wasm32")))]
pub use setup::PanicOnWarnScope;
#[cfg(feature = "setup")]
pub use setup::{setup_logging, setup_logging_with_filter};
// The tracing macros support more syntax features than the log, that's why we use them:
pub use tracing::{debug, error, info, trace, warn};

/// Log a warning in debug builds, or a debug message in release builds.
///
/// This is useful for logging messages that should be visible during development
/// (to help catch issues), but shouldn't spam the logs in release builds.
///
/// In debug builds, the message is prefixed with "DEBUG: " and logged at WARN level.
/// In release builds, the message is logged at DEBUG level without any prefix.
#[cfg(debug_assertions)]
#[macro_export]
macro_rules! debug_warn {
    ($($arg:tt)+) => {
        $crate::warn!("DEBUG: {}", format_args!($($arg)+))
    };
}

/// Log a warning in debug builds, or a debug message in release builds.
///
/// This is useful for logging messages that should be visible during development
/// (to help catch issues), but shouldn't spam the logs in release builds.
///
/// In debug builds, the message is prefixed with "DEBUG: " and logged at WARN level.
/// In release builds, the message is logged at DEBUG level without any prefix.
#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! debug_warn {
    ($($arg:tt)+) => {
        $crate::debug!($($arg)+)
    };
}

/// Like [`debug_warn!`], but only logs once per call site.
///
/// This is useful for logging messages that should be visible during development
/// (to help catch issues), but shouldn't spam the logs in release builds.
///
/// In debug builds, the message is prefixed with "DEBUG: " and logged at WARN level.
/// In release builds, the message is logged at DEBUG level without any prefix.
#[cfg(debug_assertions)]
#[macro_export]
macro_rules! debug_warn_once {
    ($($arg:tt)+) => {
        $crate::warn_once!("DEBUG: {}", format_args!($($arg)+))
    };
}

/// Like [`debug_warn!`], but only logs once per call site.
///
/// This is useful for logging messages that should be visible during development
/// (to help catch issues), but shouldn't spam the logs in release builds.
///
/// In debug builds, the message is prefixed with "DEBUG: " and logged at WARN level.
/// In release builds, the message is logged at DEBUG level without any prefix.
#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! debug_warn_once {
    ($($arg:tt)+) => {
        $crate::debug_once!($($arg)+)
    };
}

/// Re-exports of other crates.
pub mod external {
    pub use log;
}

/// Never log anything less serious than a `ERROR` from these crates.
const CRATES_AT_ERROR_LEVEL: &[&str] = &[
    // silence rustls in release mode: https://github.com/rerun-io/rerun/issues/3104
    #[cfg(not(debug_assertions))]
    "rustls",
];

/// Never log anything less serious than a `WARN` from these crates.
const CRATES_AT_WARN_LEVEL: &[&str] = &[
    // wgpu crates spam a lot on info level, which is really annoying
    // TODO(emilk): remove once https://github.com/gfx-rs/wgpu/issues/3206 is fixed
    "naga",
    "tracing",
    "wgpu_core",
    "wgpu_hal",
    "zbus",
];

/// Never log anything less serious than a `INFO` from these crates.
///
/// These creates are quite spammy on debug, drowning out what we care about:
const CRATES_AT_INFO_LEVEL: &[&str] = &[
    "datafusion_optimizer",
    "datafusion",
    "h2",
    "hyper",
    "prost_build",
    "reqwest", // Spams "starting new connection: …"
    "sqlparser",
    "tonic_web",
    "tower",
    "ureq",
    // only let rustls log in debug mode: https://github.com/rerun-io/rerun/issues/3104
    #[cfg(debug_assertions)]
    "rustls",
    // walkers generates noise around tile download, see https://github.com/podusowski/walkers/issues/199
    "walkers",
    // winit 0.30.5 spams about `set_cursor_visible` calls. It's gone on winit master, so hopefully gone in next winit release.
    "winit",
];

/// Determines the default log filter.
///
/// Native: Get `RUST_LOG` environment variable or `info`, if not set.
/// Also sets some other log levels on crates that are too loud.
///
/// Web: `debug` since web console allows arbitrary filtering.
#[cfg(not(target_arch = "wasm32"))]
pub fn default_log_filter() -> String {
    let base_log_filter = if cfg!(debug_assertions) {
        // We want the DEBUG level to be useful yet not too spammy.
        // This is a good way to enforce that.
        "debug"
    } else {
        // Important to keep the default at (at least) "info",
        // as we print crucial information at INFO,
        // e.g. the ip:port when hosting a server with `rerun-cli`.
        "info"
    };
    log_filter_from_env_or_default(base_log_filter)
}

/// Determines the default log filter.
///
/// Native: Get `RUST_LOG` environment variable or `info`, if not set.
/// Also sets some other log levels on crates that are too loud.
///
/// Web: `debug` since web console allows arbitrary filtering.
#[cfg(target_arch = "wasm32")]
pub fn default_log_filter() -> String {
    "debug".to_owned()
}

/// Determines the log filter from the `RUST_LOG` environment variable or an explicit default.
///
/// Always adds builtin filters as well.
#[cfg(not(target_arch = "wasm32"))]
pub fn log_filter_from_env_or_default(default_base_log_filter: &str) -> String {
    let rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| default_base_log_filter.to_owned());
    add_builtin_log_filter(&rust_log)
}

/// Adds builtin log level filters for crates that are too verbose.
#[cfg(not(target_arch = "wasm32"))]
fn add_builtin_log_filter(base_log_filter: &str) -> String {
    let mut rust_log = base_log_filter.to_lowercase();

    if base_log_filter != "off" {
        // If base level is `off`, don't opt-in to anything.

        for crate_name in crate::CRATES_AT_ERROR_LEVEL {
            if !rust_log.contains(&format!("{crate_name}=")) {
                rust_log += &format!(",{crate_name}=error");
            }
        }

        if base_log_filter != "error" {
            // If base level is `error`, don't opt-in to `warn` or `info`.

            for crate_name in crate::CRATES_AT_WARN_LEVEL {
                if !rust_log.contains(&format!("{crate_name}=")) {
                    rust_log += &format!(",{crate_name}=warn");
                }
            }

            if base_log_filter != "warn" {
                // If base level is not `error`/`warn`, don't opt-in to `info`.

                for crate_name in crate::CRATES_AT_INFO_LEVEL {
                    if !rust_log.contains(&format!("{crate_name}=")) {
                        rust_log += &format!(",{crate_name}=info");
                    }
                }
            }
        }
    }

    //TODO(#8077): should be removed as soon as the upstream issue is resolved
    rust_log += ",walkers::download=off";

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

/// Check if an environment variable is set to a truthy value.
///
/// Returns `true` if the environment variable is set to "1", "true", or "yes" (case-insensitive).
/// Returns `false` otherwise (including when the variable is not set).
///
/// # Example
///
/// ```ignore
/// if env_var_is_truthy("TELEMETRY_ENABLED") {
///     // enable telemetry
/// }
/// ```
pub fn env_var_is_truthy(var_name: &str) -> bool {
    std::env::var(var_name)
        .map(|v| {
            let v = v.to_lowercase();
            v == "1" || v == "true" || v == "yes"
        })
        .unwrap_or(false)
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
#[allow(clippy::allow_attributes, dead_code)] // only used on web and in tests
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
        (
            "/Users/emilk/.cargo/registry/src/github.com-1ecc6299db9ec823/tokio-1.24.1/src/runtime/runtime.rs",
            "tokio-1.24.1/src/runtime/runtime.rs",
        ),
        ("crates/rerun/src/main.rs", "rerun/src/main.rs"),
        (
            "/rustc/d5a82bbd26e1ad8b7401f6a718a9c57c96905483/library/core/src/ops/function.rs",
            "core/src/ops/function.rs",
        ),
        ("/weird/path/file.rs", "/weird/path/file.rs"),
    ] {
        assert_eq!(shorten_file_path(before), after);
    }
}
