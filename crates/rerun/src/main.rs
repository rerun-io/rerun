#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info,wgpu_core=off");
    }
    tracing_subscriber::fmt::init();

    rerun::run(std::env::args()).await
}
