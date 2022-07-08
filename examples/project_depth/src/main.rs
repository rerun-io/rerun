#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    let (rerun_tx, rerun_rx) = std::sync::mpsc::channel();
    project_depth::log(&rerun_tx);
    tracing::debug!("Starting viewer…");
    re_viewer::run_native_viewer(rerun_rx);
}
