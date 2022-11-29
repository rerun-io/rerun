use re_memory::AccountingAllocator;

#[global_allocator]
static GLOBAL: AccountingAllocator<mimalloc::MiMalloc> =
    AccountingAllocator::new(mimalloc::MiMalloc);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    re_log::set_default_rust_log_env();
    tracing_subscriber::fmt::init(); // log to stdout

    rerun::app::run(std::env::args()).await
}
