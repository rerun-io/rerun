#[cfg(not(feature = "puffin"))]
compile_error!("Feature 'puffin' must be enabled when compiling the viewer binary");

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Run viewer connected to a rerun server over websocket.
#[derive(Debug, clap::Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Start with the puffin profiler running.
    #[clap(long)]
    profile: bool,

    /// Either a path to a `.rrd` file, or an url to a websocket server.
    url_or_path: String,
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

    let path = std::path::Path::new(&args.url_or_path).to_path_buf();
    if path.exists() || args.url_or_path.ends_with(".rrd") {
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
        let mut url = args.url_or_path;
        // let url = re_ws_comms::default_server_url();
        if !url.contains("://") {
            url = format!("{}://{url}", re_ws_comms::PROTOCOL);
        }
        eframe::run_native(
            "rerun viewer",
            native_options,
            Box::new(move |cc| {
                re_viewer::customize_egui(&cc.egui_ctx);
                let mut app = re_viewer::RemoteViewerApp::new(cc.egui_ctx.clone(), cc.storage, url);
                app.set_profiler(profiler);
                Box::new(app)
            }),
        );
    }
}
