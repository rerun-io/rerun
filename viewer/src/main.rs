#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Run viewer connected to a rerun server over websocket.
#[derive(Debug, clap::Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Either a path to a `.rrd` file, or an url to a websocket server.
    url_or_path: String,
}

#[tokio::main]
async fn main() {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    use clap::Parser as _;
    let args = Args::parse();

    let native_options = eframe::NativeOptions {
        depth_buffer: 24,
        multisampling: 8,
        initial_window_size: Some([1600.0, 1200.0].into()),
        ..Default::default()
    };

    let path = std::path::Path::new(&args.url_or_path).to_path_buf();
    if path.exists() || args.url_or_path.ends_with(".rrd") {
        eframe::run_native(
            "rerun viewer",
            native_options,
            Box::new(move |cc| {
                viewer::customize_egui(&cc.egui_ctx);
                let app = viewer::App::from_rrd_path(cc.storage, &path);
                Box::new(app)
            }),
        );
    } else {
        let mut url = args.url_or_path;
        // let url = comms::default_server_url();
        if !url.contains("://") {
            url = format!("{}://{url}", comms::PROTOCOL);
        }
        eframe::run_native(
            "rerun viewer",
            native_options,
            Box::new(move |cc| {
                viewer::customize_egui(&cc.egui_ctx);
                let app = viewer::RemoteViewerApp::new(cc.egui_ctx.clone(), cc.storage, url);
                Box::new(app)
            }),
        );
    }
}
