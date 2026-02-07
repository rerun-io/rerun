//! Function to setup logging in binaries and web apps.

use std::sync::atomic::AtomicIsize;

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
    use std::str::FromStr as _;

    let primary_log_filter = log_filter.split(',').next().unwrap_or("info");
    let max_level =
        log::LevelFilter::from_str(primary_log_filter).unwrap_or(log::LevelFilter::Info);

    #[cfg(not(target_arch = "wasm32"))]
    fn setup(max_level: log::LevelFilter, log_filter: &str) {
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

        crate::multi_logger::init().expect("Failed to set logger");
        let mut stderr_logger = env_logger::Builder::new();

        log::set_max_level(max_level);

        if LOG_FILE_LINE {
            stderr_logger.format(|buf, record| {
                use std::io::Write as _;
                writeln!(
                    buf,
                    "{} {}:{} {}",
                    record.level(),
                    record.file().unwrap_or_default(),
                    record.line().unwrap_or_default(),
                    record.args()
                )
            });
        }

        stderr_logger.parse_filters(log_filter);
        crate::add_boxed_logger(Box::new(stderr_logger.build())).expect("Failed to install logger");
        crate::add_boxed_logger(Box::new(PanicOnWarn {
            always_enabled: env_var_bool("RERUN_PANIC_ON_WARN") == Some(true),
        }))
        .expect("Failed to install panic-on-warn logger");

        if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
            crate::warn!("Rerun does not officially support Intel Macs (x86/x64)");
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn setup(max_level: log::LevelFilter, _log_filter: &str) {
        crate::multi_logger::init().expect("Failed to set logger");
        log::set_max_level(max_level);
        crate::add_boxed_logger(Box::new(crate::web_logger::WebLogger::new(max_level)))
            .expect("Failed to install logger");
    }

    use std::sync::Once;
    static START: Once = Once::new();
    START.call_once(|| setup(max_level, log_filter));
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
impl log::Log for PanicOnWarn {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        match metadata.level() {
            log::Level::Error | log::Level::Warn => {
                self.always_enabled
                    || PANIC_ON_WARN_SCOPE_DEPTH
                        .with(|enabled| enabled.load(std::sync::atomic::Ordering::Relaxed) > 0)
            }
            log::Level::Info | log::Level::Debug | log::Level::Trace => false,
        }
    }

    // Panic is expected only in this method as the logger requires it while the rest of the code must be panic-free.
    #[expect(clippy::panic)]
    fn log(&self, record: &log::Record<'_>) {
        // `enabled` isn't called automatically by the `log!` macros, so we have to call it here.
        // (it is only used by `log_enabled!`)
        if self.enabled(record.metadata()) {
            let level = match record.level() {
                log::Level::Error => "error",
                log::Level::Warn => "warning",
                log::Level::Info | log::Level::Debug | log::Level::Trace => return,
            };
            panic!("{level} logged with RERUN_PANIC_ON_WARN: {}", record.args());
        }
    }

    fn flush(&self) {}
}
