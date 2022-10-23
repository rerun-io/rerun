#[global_allocator]
static GLOBAL: re_mem_tracker::TrackingAllocator<mimalloc::MiMalloc> =
    re_mem_tracker::TrackingAllocator::new(mimalloc::MiMalloc);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    rerun::run(std::env::args()).await
}
