//! The `rerun` binary, part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.
//!
//! Run `rerun --help` for more information.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!
//! ## Links
//! - [Examples](https://github.com/rerun-io/rerun/tree/latest/examples/rust)
//! - [High-level docs](http://rerun.io/docs)
//! - [Rust API docs](https://docs.rs/rerun/)
//! - [Troubleshooting](https://www.rerun.io/docs/getting-started/troubleshooting)
use re_memory::AccountingAllocator;

#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

#[tokio::main]
async fn main() -> std::process::ExitCode {
    re_log::setup_logging();

    // Name the rayon threads for the benefit of debuggers and profilers:
    rayon::ThreadPoolBuilder::new()
        .thread_name(|i| format!("rayon-{i}"))
        .build_global()
        .unwrap();

    let build_info = re_build_info::build_info!();

    let result = rerun::run(build_info, rerun::CallSource::Cli, std::env::args()).await;

    match result {
        Ok(exit_code) => std::process::ExitCode::from(exit_code),
        Err(err) => {
            // Note: we do not print the backtrace here, because our error messages should be short, readable, and actionable.
            // If we instead return an `anyhow::Result` from `main`, then the backtrace will be printed if `RUST_BACKTRACE=1`.
            eprintln!("Error: {}", re_error::format(err));
            std::process::ExitCode::FAILURE
        }
    }
}
