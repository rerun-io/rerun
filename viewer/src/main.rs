/// Run viewer connected to a rerun server over websocket.
#[derive(Debug, clap::Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Url to server
    #[clap(long, default_value = "ws://127.0.0.1:9876")]
    url: String,
}

#[tokio::main]
async fn main() {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    use clap::Parser as _;
    let args = Args::parse();

    let mut url = args.url;
    // let url = comms::default_server_url();

    if !url.contains("://") {
        url = format!("{}://{url}", comms::PROTOCOL);
    }

    let native_options = eframe::NativeOptions {
        depth_buffer: 24,
        multisampling: 8,
        ..Default::default()
    };

    eframe::run_native(
        "rerun viewer",
        native_options,
        Box::new(move |cc| {
            let app = viewer::RemoteViewerApp::new(cc.egui_ctx.clone(), cc.storage, url);
            Box::new(app)
        }),
    );
}
