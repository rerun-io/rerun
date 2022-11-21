use re_memory::TrackingAllocator;

#[global_allocator]
static GLOBAL: TrackingAllocator<mimalloc::MiMalloc> = TrackingAllocator::new(mimalloc::MiMalloc);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        // Enable logging unless the user opts-out of it.
        std::env::set_var("RUST_LOG", "info,wgpu_core=warn,wgpu_hal=warn");
    }
    tracing_subscriber::fmt::init(); // log to stdout

    rerun::run(std::env::args()).await
}
