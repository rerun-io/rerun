//! Integrates the Rerun SDK with the [`log`] crate.

use log::Log as _;
use re_sdk_types::archetypes::TextLog;
use re_sdk_types::components::TextLogLevel;

use crate::RecordingStream;

// ---

/// Implements a [`log::Log`] that forwards all events to the Rerun SDK.
///
/// ```
/// let rec = rerun::RecordingStreamBuilder::new("rerun_example_app").buffered()?;
///
/// rerun::Logger::new(rec.clone()) // recording streams are ref-counted
///     .with_path_prefix("logs")
///     .with_filter(rerun::default_log_filter())
///     .init()?;
///
/// log::info!("This INFO log got added through the standard logging interface");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct Logger {
    rec: RecordingStream,
    filter: Option<env_filter::Filter>,
    path_prefix: Option<String>,
}

impl Drop for Logger {
    fn drop(&mut self) {
        self.flush();
    }
}

impl Logger {
    /// Returns a new [`Logger`] that forwards all events to the specified [`RecordingStream`].
    pub fn new(rec: RecordingStream) -> Self {
        Self {
            rec,
            filter: None,
            path_prefix: None,
        }
    }

    /// Configures the [`Logger`] to prefix the specified `path_prefix` to all events.
    #[inline]
    pub fn with_path_prefix(mut self, path_prefix: impl Into<String>) -> Self {
        self.path_prefix = Some(path_prefix.into());
        self
    }

    /// Configures the [`Logger`] to filter events.
    ///
    /// This uses the familiar [env_logger syntax].
    ///
    /// If you don't call this, the [`Logger`] will parse the `RUST_LOG` environment variable
    /// instead when you [`Logger::init`] it.
    ///
    /// [env_logger syntax]: https://docs.rs/env_logger/latest/env_logger/index.html#enabling-logging
    #[inline]
    pub fn with_filter(mut self, filter: impl AsRef<str>) -> Self {
        use env_filter::Builder;
        self.filter = Some(Builder::new().parse(filter.as_ref()).build());
        self
    }

    /// Sets the [`Logger`] as global logger.
    ///
    /// All calls to [`log`] macros will go through this [`Logger`] from this point on.
    pub fn init(mut self) -> Result<(), log::SetLoggerError> {
        if self.filter.is_none() {
            use env_filter::Builder;
            self.filter = Some(Builder::new().parse(&re_log::default_log_filter()).build());
        }

        // NOTE: We will have to make filtering decisions on a per-crate/module basis, therefore
        // there is no global filtering ceiling.
        log::set_max_level(log::LevelFilter::max());
        log::set_boxed_logger(Box::new(self))
    }
}

impl log::Log for Logger {
    #[inline]
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        self.filter
            .as_ref()
            .is_none_or(|filter| filter.enabled(metadata))
    }

    #[inline]
    fn log(&self, record: &log::Record<'_>) {
        if !self
            .filter
            .as_ref()
            .is_none_or(|filter| filter.matches(record))
        {
            return;
        }

        let target = record.metadata().target().replace("::", "/");
        let ent_path = if let Some(path_prefix) = self.path_prefix.as_ref() {
            format!("{path_prefix}/{target}")
        } else {
            target
        };

        let level = log_level_to_rerun_level(record.metadata().level());

        let body = format!("{}", record.args());

        self.rec
            .log(ent_path, &TextLog::new(body).with_level(level))
            .ok(); // ignore error
    }

    #[inline]
    fn flush(&self) {
        self.rec.flush_blocking().ok();
    }
}

// ---

fn log_level_to_rerun_level(lvl: log::Level) -> TextLogLevel {
    match lvl {
        log::Level::Error => TextLogLevel::ERROR,
        log::Level::Warn => TextLogLevel::WARN,
        log::Level::Info => TextLogLevel::INFO,
        log::Level::Debug => TextLogLevel::DEBUG,
        log::Level::Trace => TextLogLevel::TRACE,
    }
    .into()
}
