#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Visualize an NYU Depth v2 data instance.
#[derive(Debug, clap::Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path(s) to the data
    dirs: Vec<std::path::PathBuf>,

    /// Serve web viewer (instead of running a native viewer directly).
    #[cfg(feature = "web")]
    #[clap(long)]
    web: bool,

    /// Open the web viewer directly.
    #[cfg(feature = "web")]
    #[clap(long)]
    open: bool,

    /// Start with the puffin profiler running.
    #[clap(long)]
    profile: bool,
}

#[cfg(not(feature = "web"))]
fn main() {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    use clap::Parser as _;
    let args = Args::parse();
    assert!(
        !args.dirs.is_empty(),
        "Requires at least one file directory"
    );

    let mut profiler = re_viewer::Profiler::default();
    if args.profile {
        profiler.start();
    }

    let (rerun_tx, rerun_rx) = std::sync::mpsc::channel();

    let handle = std::thread::spawn(move || {
        for dir in &args.dirs {
            nyud::log_dataset(dir, &rerun_tx).unwrap();
        }
    });

    tracing::debug!("Starting viewer…");
    re_viewer::run_native_viewer(rerun_rx);
    handle.join().ok();
}

#[cfg(feature = "web")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    use clap::Parser as _;
    let args = Args::parse();
    assert!(
        !args.dirs.is_empty(),
        "Requires at least one file directory"
    );

    let (rerun_tx, rerun_rx) = std::sync::mpsc::channel();

    let handle = std::thread::spawn(move || {
        for dir in &args.dirs {
            nyud::log_dataset(dir, &rerun_tx).unwrap();
        }
    });

    let pub_sub_url = re_comms::default_server_url();

    let server = re_comms::Server::new(re_comms::DEFAULT_SERVER_PORT).await?;
    let server_handle = tokio::spawn(server.listen(rerun_rx));

    let web_port = 9090;

    let web_server_handle = tokio::spawn(async move {
        re_web_server::run(web_port).await.unwrap();
    });

    let viewer_url = format!("http://127.0.0.1:{}?url={}", web_port, pub_sub_url);
    if args.open {
        std::thread::sleep(std::time::Duration::from_millis(1000)); // give web server time to start
        webbrowser::open(&viewer_url).ok();
    } else {
        println!("Web server is running - view it at {}", viewer_url);
    }

    server_handle.await??;
    web_server_handle.await?;
    handle.join().ok();
    Ok(())
}
