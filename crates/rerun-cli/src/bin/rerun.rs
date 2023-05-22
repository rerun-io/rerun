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
