//! Function to setup logging in binaries and web apps.

use std::sync::atomic::AtomicIsize;
use tracing_subscriber::prelude::*;

// This can be useful to enable to figure out what is causing a log message.
#[cfg(not(target_arch = "wasm32"))]
const LOG_FILE_LINE: bool = false;

/// Sets up logging for the current process using default log filter as defined in `crate::default_log_filter`.
///
/// Automatically does the right thing depending on target environment (native vs. web).
/// Directs [`log`] calls to stderr on native.
pub fn setup_logging() {
    setup_logging_with_filter(&crate::default_log_filter());
}

/// Sets up logging for the current process using an explicit log filter.
///
/// Automatically does the right thing depending on target environment (native vs. web).
/// Directs [`log`] calls to stderr on native.
pub fn setup_logging_with_filter(log_filter: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    fn create_tracing_subscriber(log_filter: &str) -> impl tracing::Subscriber {
        if cfg!(debug_assertions) && std::env::var("RUST_BACKTRACE").is_err() {
            // In debug build, default `RUST_BACKTRACE` to `1` if it is not set.
            // This ensures sure we produce backtraces if our examples (etc) panics.

            // Our own crash handler (`re_crash_handler`) always prints a backtraces
            // (currently ignoring `RUST_BACKTRACE`) but we only use that for `rerun-cli`, our main binary.

            // `RUST_BACKTRACE` also turns on printing backtraces for `anyhow::Error`s that
            // are returned from `main` (i.e. if `main` returns `anyhow::Result`).

            // SAFETY: the chances of this causing problems are slim
            #[expect(unsafe_code)]
            unsafe {
                std::env::set_var("RUST_BACKTRACE", "1"); // TODO(emilk): There should be a better way to do this.
            }
        }

        let fmt_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_file(LOG_FILE_LINE)
            .with_line_number(LOG_FILE_LINE);
        let env_filter = tracing_subscriber::EnvFilter::new(log_filter);
        let panic_on_warn = PanicOnWarn {
            always_enabled: env_var_bool("RERUN_PANIC_ON_WARN") == Some(true),
        };

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(panic_on_warn)
            .with(crate::channel_logger::channel_logger())
    }

    #[cfg(target_arch = "wasm32")]
    fn create_tracing_subscriber(_log_filter: &str) -> impl tracing::Subscriber {
        let fmt_layer = tracing_subscriber::Layer::with_filter(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .without_time()
                .with_writer(tracing_web::MakeWebConsoleWriter::new()),
            // Cap output to DEBUG since browsers don't have a trace level in the web console.
            tracing_subscriber::filter::LevelFilter::DEBUG,
        );

        tracing_subscriber::registry()
            .with(fmt_layer)
            .with(crate::channel_logger::channel_logger())
    }

    use std::sync::Once;
    static START: Once = Once::new();
    START.call_once(|| {
        use std::str::FromStr as _;

        if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
            crate::warn!("Rerun does not officially support Intel Macs (x86/x64)");
        }

        let primary_log_filter = log_filter.split(',').next().unwrap_or("info");
        let max_level =
            log::LevelFilter::from_str(primary_log_filter).unwrap_or(log::LevelFilter::Info);
        log::set_max_level(max_level);

        // Forward `log` calls to `tracing`, so that if a dependency uses `log` instead of `tracing`,
        // the log messages will still be captured by our `tracing` setup.
        if let Err(err) = tracing_log::LogTracer::init() {
            eprintln!("Failed to set log to tracing forwarding: {err}");
        }

        let subscriber = create_tracing_subscriber(log_filter);
        if tracing::subscriber::set_global_default(subscriber).is_err() {
            eprintln!(
                "Failed to set global tracing subscriber. This can cause problems with log messages not being captured."
            );
            crate::debug_panic!(
                "Failed to set global tracing subscriber. This can cause problems with log messages not being captured."
            );
        }
    });
}

// ----------------------------------------------------------------------------

thread_local! {
    static PANIC_ON_WARN_SCOPE_DEPTH: AtomicIsize = const { AtomicIsize::new(0) };
}

/// Scope for enabling panic on warn/error log messages temporariliy on the current thread (!).
///
/// Use this in tests to ensure that there's no errors & warnings.
/// Note that we can't enable this for all threads since threads run in parallel and may not want to set this.
#[cfg(not(target_arch = "wasm32"))]
pub struct PanicOnWarnScope {
    // The panic scope should decrease the same thread-local value, so it mustn't be Send or Sync.
    not_send_sync: std::marker::PhantomData<std::cell::Cell<()>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl PanicOnWarnScope {
    /// Enable panic on warn & error log messages for as long as this scope is alive.
    #[expect(clippy::new_without_default)]
    pub fn new() -> Self {
        PANIC_ON_WARN_SCOPE_DEPTH.with(|enabled| {
            enabled.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });
        Self {
            not_send_sync: Default::default(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for PanicOnWarnScope {
    fn drop(&mut self) {
        PANIC_ON_WARN_SCOPE_DEPTH.with(|enabled| {
            enabled.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn env_var_bool(name: &str) -> Option<bool> {
    let s = std::env::var(name).ok()?;
    match s.to_lowercase().as_str() {
        "0" | "false" | "off" | "no" => Some(false),
        "1" | "true" | "on" | "yes" => Some(true),
        _ => {
            crate::warn!(
                "Invalid value for environment variable {name}={s:?}. Expected 'on' or 'off'. It will be ignored"
            );
            None
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct PanicOnWarn {
    always_enabled: bool,
}

#[cfg(not(target_arch = "wasm32"))]
impl<S> tracing_subscriber::Layer<S> for PanicOnWarn
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let level = match *metadata.level() {
            tracing::Level::ERROR => "error",
            tracing::Level::WARN => "warning",
            tracing::Level::INFO | tracing::Level::DEBUG | tracing::Level::TRACE => return,
        };

        let enabled = self.always_enabled
            || PANIC_ON_WARN_SCOPE_DEPTH
                .with(|enabled| enabled.load(std::sync::atomic::Ordering::Relaxed) > 0);
        if enabled {
            let mut visitor = crate::event_visitor::EventVisitor::default();
            event.record(&mut visitor);
            panic!(
                "{level} logged with RERUN_PANIC_ON_WARN: {}",
                visitor.finish()
            );
        }
    }
}
