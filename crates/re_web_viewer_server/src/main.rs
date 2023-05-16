#![forbid(unsafe_code)]
#![warn(clippy::all, rust_2018_idioms)]

/// Build and host the rerun web-viewer.
///
/// Debug-builds will build and host the debug version of the viewer.
#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    /// What TCP port do we listen to?
    ///
    /// Use `0` to tell the OS to use any port.
    #[clap(long, default_value_t = 0)]
    port: u16,

    /// What bind address IP to use.
    #[clap(long, default_value = "0.0.0.0")]
    bind: String,
}

#[tokio::main]
async fn main() {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    // Shutdown server via Ctrl+C
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    ctrlc::set_handler(move || {
        re_log::debug!("Ctrl-C detected - Closing web server.");
        shutdown_tx.send(()).unwrap();
    })
    .expect("Error setting Ctrl-C handler");

    let bind_ip = &args.bind;
    let server = re_web_viewer_server::WebViewerServer::new(
        bind_ip,
        re_web_viewer_server::WebViewerServerPort(args.port),
    )
    .expect("Could not create web server");

    let port = server.port();
    eprintln!("Hosting web-viewer on http://{bind_ip}:{port}");

    server.serve(shutdown_rx).await.unwrap();
}
