#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// The Rerun Viewer and Server
///
/// Features:
///
/// * Read `.rrd` (rerun recording files).
/// * Connect to a Rerun Server over web-sockets.
/// * Host a Rerun Server that Rerun SDK:s can connect to.
#[derive(Debug, clap::Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Either a path to a `.rrd` file, or a websocket url to a Rerun Server.
    ///
    /// If none is given, a server will be hosted which the Rerun SDK can connect to.
    url_or_path: Option<String>,

    /// When using `--host`, what port do we listen on?
    #[cfg(feature = "server")]
    #[clap(long, default_value_t = re_sdk_comms::DEFAULT_SERVER_PORT)]
    port: u16,

    /// Start with the puffin profiler running.
    #[clap(long)]
    profile: bool,
}

#[tokio::main]
async fn main() {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    use clap::Parser as _;
    let args = Args::parse();

    let mut profiler = re_viewer::Profiler::default();
    if args.profile {
        profiler.start();
    }

    let native_options = eframe::NativeOptions {
        depth_buffer: 24,
        multisampling: 8,
        initial_window_size: Some([1600.0, 1200.0].into()),
        ..Default::default()
    };

    if let Some(url_or_path) = &args.url_or_path {
        let path = std::path::Path::new(url_or_path).to_path_buf();
        if path.exists() || url_or_path.ends_with(".rrd") {
            eframe::run_native(
                "rerun viewer",
                native_options,
                Box::new(move |cc| {
                    re_viewer::customize_egui(&cc.egui_ctx);
                    let mut app = re_viewer::App::from_rrd_path(cc.storage, &path);
                    app.set_profiler(profiler);
                    Box::new(app)
                }),
            );
        } else {
            let mut url = url_or_path.clone();
            // let url = re_ws_comms::default_server_url();
            if !url.contains("://") {
                url = format!("{}://{url}", re_ws_comms::PROTOCOL);
            }
            eframe::run_native(
                "rerun viewer",
                native_options,
                Box::new(move |cc| {
                    re_viewer::customize_egui(&cc.egui_ctx);
                    let mut app =
                        re_viewer::RemoteViewerApp::new(cc.egui_ctx.clone(), cc.storage, url);
                    app.set_profiler(profiler);
                    Box::new(app)
                }),
            );
        }
    } else {
        #[cfg(feature = "server")]
        {
            let bind_addr = format!("127.0.0.1:{}", args.port);
            match re_sdk_comms::serve(&bind_addr) {
                Ok(rx) => {
                    tracing::info!("Hosting SDK server on {bind_addr}");
                    re_viewer::run_native_viewer(rx);
                }
                Err(err) => {
                    panic!("Failed to host: {err}");
                }
            }
        }

        #[cfg(not(feature = "server"))]
        panic!("No url or .rrd path given");
    }
}
