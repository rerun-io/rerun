//! Function to setup logging in binaries and web apps.

/// Set `RUST_LOG` environment variable to `info`, unless set,
/// and also set some other log levels on crates that are too loud.
#[cfg(not(target_arch = "wasm32"))]
fn set_default_rust_log_env() {
    let mut rust_log = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned());

    const LOUD_CRATES: [&str; 7] = [
        // wgpu crates spam a lot on info level, which is really annoying
        // TODO(emilk): remove once https://github.com/gfx-rs/wgpu/issues/3206 is fixed
        "naga",
        "wgpu_core",
        "wgpu_hal",
        // These are quite spammy on debug, drowning out what we care about:
        "h2",
        "hyper",
        "rustls",
        "ureq",
    ];
    for loud_crate in LOUD_CRATES {
        if !rust_log.contains(&format!("{loud_crate}=")) {
            rust_log += &format!(",{loud_crate}=warn");
        }
    }

    std::env::set_var("RUST_LOG", rust_log);

    if std::env::var("RUST_BACKTRACE").is_err() {
        // Make sure we always produce backtraces for the (hopefully rare) cases when we crash!
        std::env::set_var("RUST_BACKTRACE", "1");
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn setup_native_logging() {
    set_default_rust_log_env();
    log::set_max_level(log::LevelFilter::Debug);
    crate::multi_logger::init().expect("Failed to set logger");
    crate::add_boxed_logger(Box::new(env_logger::Builder::from_env("RUST_LOG").build()));
}

#[cfg(target_arch = "wasm32")]
pub fn setup_web_logging() {
    log::set_max_level(log::LevelFilter::Debug);
    crate::multi_logger::init().expect("Failed to set logger");
    crate::add_boxed_logger(Box::new(crate::web_logger::WebLogger::new(
        log::LevelFilter::Debug,
    )))
}
