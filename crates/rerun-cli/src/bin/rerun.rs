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
async fn main() -> anyhow::Result<std::process::ExitCode> {
    re_log::setup_native_logging();
    let build_info = re_build_info::build_info!();
    rerun::run(build_info, rerun::CallSource::Cli, std::env::args())
        .await
        .map(std::process::ExitCode::from)
}
