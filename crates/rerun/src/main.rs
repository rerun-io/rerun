use re_memory::AccountingAllocator;

#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

#[tokio::main]
async fn main() -> anyhow::Result<std::process::ExitCode> {
    re_log::setup_native_logging();
    let build_info = re_build_info::build_info!();
    depthai_viewer::run(
        build_info,
        depthai_viewer::CallSource::Cli,
        std::env::args(),
    )
    .await
    .map(std::process::ExitCode::from)
}
