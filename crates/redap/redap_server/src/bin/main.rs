use re_memory::AccountingAllocator;

#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

fn main() -> std::process::ExitCode {
    re_log::setup_logging();

    let result = redap_server::run(std::env::args());

    match result {
        Ok(_) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            // Note: we do not print the backtrace here, because our error messages should be short, readable, and actionable.
            // If we instead return an `anyhow::Result` from `main`, then the backtrace will be printed if `RUST_BACKTRACE=1`.
            eprintln!("Error: {}", re_error::format(err));
            std::process::ExitCode::FAILURE
        }
    }
}
