//! Function to setup logging in binaries and web apps.

/// Get `RUST_LOG` environment variable or `info`, if not set.
///
/// Also set some other log levels on crates that are too loud.
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
    for crate_name in crate::CRATES_FORCED_TO_INFO {
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
}

#[cfg(target_arch = "wasm32")]
pub fn setup_web_logging() {
    crate::multi_logger::init().expect("Failed to set logger");
    log::set_max_level(log::LevelFilter::Debug);
    crate::add_boxed_logger(Box::new(crate::web_logger::WebLogger::new(
        log::LevelFilter::Debug,
    )))
    .expect("Failed to install logger");
}
