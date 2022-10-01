//! The main Rerun binary.
//!
//! This can act either as a server, a viewer, or both, depending on which options you use when you start it.

use std::sync::mpsc::Receiver;

use re_log_types::LogMsg;

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
    /// Either:
    /// * a path to a `.rrd` file to load
    /// * a websocket url to a Rerun Server from which to read data
    ///
    /// If none is given, a server will be hosted which the Rerun SDK can connect to.
    url_or_path: Option<String>,

    /// What TCP port do we listen to (for SDK:s to connect to)?
    #[cfg(feature = "server")]
    #[clap(long, default_value_t = re_sdk_comms::DEFAULT_SERVER_PORT)]
    port: u16,

    /// Start the viewer in the browser (instead of locally).
    /// Requires rerun to have been compiled with the 'web' feature.
    #[clap(long)]
    web_viewer: bool,

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

    // Where do we get the data from?
    let rx = if let Some(url_or_path) = &args.url_or_path {
        let path = std::path::Path::new(url_or_path).to_path_buf();
        if path.exists() || url_or_path.ends_with(".rrd") {
            re_log::info!("Loading {path:?}…");
            load_file_to_channel(&path)
                .unwrap_or_else(|err| panic!("Failed to load {path:?}: {}", re_error::format(err)))
        } else {
            connect_to_ws_url(&args, profiler, url_or_path.clone()).await;
            return;
        }
    } else {
        #[cfg(feature = "server")]
        {
            let bind_addr = format!("127.0.0.1:{}", args.port);
            let rx = re_sdk_comms::serve(&bind_addr).unwrap_or_else(|err| {
                panic!("Failed to host: {}", re_error::format(err));
            });
            re_log::info!("Hosting a SDK server over TCP at {bind_addr}");
            rx
        }

        #[cfg(not(feature = "server"))]
        panic!("No url or .rrd path given");
    };

    // Now what do we do with the data?
    if args.web_viewer {
        #[cfg(feature = "web")]
        {
            // This is the server which the web viewer will talk to:
            re_log::info!("Starting a Rerun WebSocket Server…");
            let ws_server = re_ws_comms::Server::new(re_ws_comms::DEFAULT_WS_SERVER_PORT)
                .await
                .unwrap();
            let server_handle = tokio::spawn(ws_server.listen(rx));

            let rerun_ws_server_url = re_ws_comms::default_server_url();
            host_web_viewer(rerun_ws_server_url).await;

            server_handle.await.unwrap().unwrap();
        }

        #[cfg(not(feature = "web"))]
        panic!("Can't host web-viewer - rerun was not compiled with the 'web' feature");
    } else {
        re_viewer::run_native_app(Box::new(move |cc| {
            let rx = re_viewer::wake_up_ui_thread_on_each_msg(rx, cc.egui_ctx.clone());
            let mut app = re_viewer::App::from_receiver(&cc.egui_ctx, cc.storage, rx);
            app.set_profiler(profiler);
            Box::new(app)
        }));
    }
}

async fn connect_to_ws_url(
    args: &Args,
    profiler: re_viewer::Profiler,
    mut rerun_server_ws_url: String,
) {
    if !rerun_server_ws_url.contains("://") {
        rerun_server_ws_url = format!("{}://{rerun_server_ws_url}", re_ws_comms::PROTOCOL);
    }

    if args.web_viewer {
        host_web_viewer(rerun_server_ws_url).await;
    } else {
        // By using RemoteViewerApp we let the user change the server they are connected to.
        re_viewer::run_native_app(Box::new(move |cc| {
            let mut app =
                re_viewer::RemoteViewerApp::new(&cc.egui_ctx, cc.storage, rerun_server_ws_url);
            app.set_profiler(profiler);
            Box::new(app)
        }));
    }
}

fn load_file_to_channel(path: &std::path::Path) -> anyhow::Result<Receiver<LogMsg>> {
    use anyhow::Context as _;
    let file = std::fs::File::open(path).context("Failed to open file")?;
    let decoder = re_log_types::encoding::Decoder::new(file)?;

    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::Builder::new()
        .name("rrd_file_reader".into())
        .spawn(move || {
            for msg in decoder {
                tx.send(msg.unwrap()).ok();
            }
        })
        .expect("Failed to spawn thread");

    Ok(rx)
}

#[cfg(feature = "web")]
async fn host_web_viewer(rerun_ws_server_url: String) {
    let web_port = 9090;
    let viewer_url = format!("http://127.0.0.1:{}?url={}", web_port, rerun_ws_server_url);

    let web_server = re_web_server::WebServer::new(web_port);
    let web_server_handle = tokio::spawn(async move {
        web_server.serve().await.unwrap();
    });

    let open = true;
    if open {
        webbrowser::open(&viewer_url).ok();
    } else {
        println!("Hosting Rerun Web Viewer at {viewer_url}.");
    }

    web_server_handle.await.unwrap();
}

#[cfg(not(feature = "web"))]
async fn host_web_viewer(rerun_ws_server_url: String) {
    panic!("Can't host web-viewer - rerun was not compiled with the 'web' feature");
}
