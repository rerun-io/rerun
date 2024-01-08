//! Function to setup logging in binaries and web apps.

/// Get `RUST_LOG` environment variable or `info`, if not set.
///
/// Also sets some other log levels on crates that are too loud.
#[cfg(not(target_arch = "wasm32"))]
pub fn default_log_filter() -> String {
    let mut rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned());

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

/// Directs [`log`] calls to stderr.
#[cfg(not(target_arch = "wasm32"))]
pub fn setup_native_logging() {
    if std::env::var("RUST_BACKTRACE").is_err() {
        // Make sure we always produce backtraces for the (hopefully rare) cases when we crash!
        std::env::set_var("RUST_BACKTRACE", "1");
    }

    crate::multi_logger::init().expect("Failed to set logger");

    let log_filter = default_log_filter();

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

#[cfg(target_arch = "wasm32")]
pub fn setup_web_logging() {
    use std::sync::atomic::{AtomicBool, Ordering};

    static LOG_INIT: AtomicBool = AtomicBool::new(false);
    if LOG_INIT.load(Ordering::SeqCst) {
        return;
    }

    crate::multi_logger::init().expect("Failed to set logger");
    log::set_max_level(log::LevelFilter::Debug);
    crate::add_boxed_logger(Box::new(crate::web_logger::WebLogger::new(
        log::LevelFilter::Debug,
    )))
    .expect("Failed to install logger");

    LOG_INIT.store(true, Ordering::SeqCst);
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
