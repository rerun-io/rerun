//! Text logging (nothing to do with rerun logging) for use in rerun libraries.
//!
//! * `trace`: spammy things
//! * `debug`: things that might be useful when debugging
//! * `info`: things that we want to show to users
//! * `warn`: problems that we can recover from
//! * `error`: problems that lead to loss of functionality or data
//!
//! The `warn_once` etc macros are for when you want to suppress repeated
//! logging of the exact same message.

// The tracing macros support more syntax features than the log, that's why we use them:
pub use tracing::{debug, error, info, trace, warn};

// The `re_log::info_once!(…)` etc are nice helpers, but the `log-once` crate is a bit lacking.
// In the future we should implement our own macros to de-duplicate based on the callsite,
// similar to how the log console in a browser will automatically suppress duplicates.
pub use log_once::{debug_once, error_once, info_once, trace_once, warn_once};

mod setup;

pub use setup::*;

pub use log::{Level, LevelFilter};

mod multi_logger;

pub use multi_logger::{add_boxed_logger, add_logger};

#[cfg(target_arch = "wasm32")]
mod log_web;

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

// ----------------------------------------------------------------------------

pub struct LogMsg {
    pub level: Level,
    pub msg: String,
}

/// Pipe log messages to a channel.
pub struct ChannelLogger {
    filter: log::LevelFilter,
    tx: parking_lot::Mutex<std::sync::mpsc::Sender<LogMsg>>,
}

impl ChannelLogger {
    pub fn new(filter: log::LevelFilter) -> (Self, std::sync::mpsc::Receiver<LogMsg>) {
        let (tx, rx) = std::sync::mpsc::channel();
        (
            Self {
                filter,
                tx: tx.into(),
            },
            rx,
        )
    }
}

impl log::Log for ChannelLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        if metadata.target().starts_with("wgpu") || metadata.target().starts_with("naga") {
            // TODO(emilk): remove once https://github.com/gfx-rs/wgpu/issues/3206 is fixed
            return metadata.level() <= log::LevelFilter::Warn;
        }

        metadata.level() <= self.filter
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        self.tx
            .lock()
            .send(LogMsg {
                level: record.level(),
                msg: record.args().to_string(),
            })
            .ok();
    }

    fn flush(&self) {}
}
