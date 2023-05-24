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

    /// Open the web-viewer in the default browser?
    #[clap(long)]
    open: bool,
}

#[tokio::main]
async fn main() {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let bind_ip = &args.bind;
    let server = re_web_viewer_server::WebViewerServer::new(
        bind_ip,
        re_web_viewer_server::WebViewerServerPort(args.port),
    )
    .expect("Could not create web server");

    let url = server.server_url();
    eprintln!("Hosting web-viewer on {url}");

    if args.open {
        if let Err(err) = webbrowser::open(&url) {
            re_log::error!("Could not open web browser: {err}");
        }
    }

    server.serve().await.unwrap();
}
