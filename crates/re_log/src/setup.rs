//! Function to setup logging in binaries and web apps.

/// Directs [`log`] calls to stderr.
#[cfg(not(target_arch = "wasm32"))]
pub fn setup_native_logging() {
    fn setup() {
        if cfg!(debug_assertions) && std::env::var("RUST_BACKTRACE").is_err() {
            // In debug build, default `RUST_BACKTRACE` to `1` if it is not set.
            // This ensures sure we produce backtraces if our examples (etc) panics.

            // Our own crash handler (`re_crash_handler`) always prints a backtraces
            // (currently ignoring `RUST_BACKTRACE`) but we only use that for `rerun-cli`, our main binary.

            // `RUST_BACKTRACE` also turns on printing backtraces for `anyhow::Error`s that
            // are returned from `main` (i.e. if `main` returns `anyhow::Result`).
            std::env::set_var("RUST_BACKTRACE", "1");
        }

        crate::multi_logger::init().expect("Failed to set logger");

        let log_filter = crate::default_log_filter();

        if log_filter.contains("trace") {
            log::set_max_level(log::LevelFilter::Trace);
        } else if log_filter.contains("debug") {
            log::set_max_level(log::LevelFilter::Debug);
        } else {
            log::set_max_level(log::LevelFilter::Info);
        }

        let mut stderr_logger = env_logger::Builder::new();
        stderr_logger.parse_filters(&log_filter);
        crate::add_boxed_logger(Box::new(stderr_logger.build())).expect("Failed to install logger");

        if env_var_bool("RERUN_PANIC_ON_WARN") == Some(true) {
            crate::add_boxed_logger(Box::new(PanicOnWarn {}))
                .expect("Failed to enable RERUN_PANIC_ON_WARN");
            crate::info!("RERUN_PANIC_ON_WARN: any warning or error will cause Rerun to panic.");
        }
    }

    use std::sync::Once;
    static START: Once = Once::new();
    START.call_once(setup);
}

#[cfg(target_arch = "wasm32")]
pub fn setup_web_logging() {
    use std::sync::atomic::{AtomicBool, Ordering};

    static LOG_INIT: AtomicBool = AtomicBool::new(false);
    if LOG_INIT.load(Ordering::SeqCst) {
        return;
    }
    LOG_INIT.store(true, Ordering::SeqCst);

    crate::multi_logger::init().expect("Failed to set logger");
    log::set_max_level(log::LevelFilter::Debug);
    crate::add_boxed_logger(Box::new(crate::web_logger::WebLogger::new(
        log::LevelFilter::Debug,
    )))
    .expect("Failed to install logger");
}

// ----------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
fn env_var_bool(name: &str) -> Option<bool> {
    std::env::var(name).ok()
        .and_then(|s| match s.to_lowercase().as_str() {
            "0" | "false" | "off" | "no" => Some(false),
            "1" | "true" | "on" | "yes" => Some(true),
            _ => {
                crate::warn!(
                    "Invalid value for environment variable {name}={s:?}. Expected 'on' or 'off'. It will be ignored"
                );
                None
            }
        })
}

#[cfg(not(target_arch = "wasm32"))]
struct PanicOnWarn {}

#[cfg(not(target_arch = "wasm32"))]
impl log::Log for PanicOnWarn {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        match metadata.level() {
            log::Level::Error | log::Level::Warn => true,
            log::Level::Info | log::Level::Debug | log::Level::Trace => false,
        }
    }

    fn log(&self, record: &log::Record<'_>) {
        let level = match record.level() {
            log::Level::Error => "error",
            log::Level::Warn => "warning",
            log::Level::Info | log::Level::Debug | log::Level::Trace => return,
        };

        panic!("{level} logged with RERUN_PANIC_ON_WARN: {}", record.args());
    }

    fn flush(&self) {}
}
