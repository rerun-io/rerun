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

pub use tracing::{debug, error, info, trace, warn};

// The `re_log::info_once!(…)` etc are nice helpers, but the `log-once` crate is a bit lacking.
// In the future we should implement our own `tracing` layer and de-duplicate based on the callsite,
// similar to how the log console in a browser will automatically suppress duplicates.
pub use log_once::{debug_once, error_once, info_once, trace_once, warn_once};

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
        "reqwest",
        "rustls",
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
    tracing_subscriber::fmt::init(); // log to stdout
}

#[cfg(target_arch = "wasm32")]
fn default_web_log_filter() -> String {
    "debug,naga=warn,wgpu_core=warn,wgpu_hal=warn".to_owned()
}

#[cfg(target_arch = "wasm32")]
pub fn setup_web_logging() {
    use tracing_subscriber::layer::SubscriberExt as _;
    tracing::subscriber::set_global_default(
        tracing_subscriber::Registry::default()
            .with(tracing_subscriber::EnvFilter::new(default_web_log_filter()))
            .with(tracing_wasm::WASMLayer::new(
                tracing_wasm::WASMLayerConfig::default(),
            )),
    )
    .expect("Failed to set tracing subscriber.");
}
