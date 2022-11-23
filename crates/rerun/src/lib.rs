//! The main Rerun binary.
//!
//! This can act either as a server, a viewer, or both, depending on which options you use when you start it.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

use anyhow::Context;
use std::sync::mpsc::Receiver;

use re_log_types::LogMsg;

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
    /// Either a path to a `.rrd` file to load, or a websocket url to a Rerun Server from which to read data
    ///
    /// If none is given, a server will be hosted which the Rerun SDK can connect to.
    url_or_path: Option<String>,

    /// What TCP port do we listen to (for SDK:s to connect to)?
    #[cfg(feature = "server")]
    #[clap(long, default_value_t = re_sdk_comms::DEFAULT_SERVER_PORT)]
    port: u16,

    /// Start the viewer in the browser (instead of locally).
    /// Requires Rerun to have been compiled with the 'web' feature.
    #[clap(long)]
    web_viewer: bool,

    /// Start with the puffin profiler running.
    #[clap(long)]
    profile: bool,
}

pub async fn run<I, T>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    re_memory::tracking_allocator::turn_on_tracking_if_env_var(
        re_viewer::env_vars::RERUN_TRACK_ALLOCATIONS,
    );

    use clap::Parser as _;
    let args = Args::parse_from(args);
    run_impl(args).await
}

async fn run_impl(args: Args) -> anyhow::Result<()> {
    let mut profiler = re_viewer::Profiler::default();
    if args.profile {
        profiler.start();
    }

    // Where do we get the data from?
    let rx = if let Some(url_or_path) = &args.url_or_path {
        let path = std::path::Path::new(url_or_path).to_path_buf();
        if path.exists() || url_or_path.ends_with(".rrd") {
            re_log::info!("Loading {path:?}…");
            load_file_to_channel(&path).with_context(|| format!("{path:?}"))?
        } else {
            return connect_to_ws_url(&args, profiler, url_or_path.clone()).await;
        }
    } else {
        #[cfg(feature = "server")]
        {
            let bind_addr = format!("127.0.0.1:{}", args.port);
            let rx = re_sdk_comms::serve(&bind_addr)
                .with_context(|| format!("Failed to bind address {bind_addr:?}"))?;
            re_log::info!("Hosting a SDK server over TCP at {bind_addr}");
            rx
        }

        #[cfg(not(feature = "server"))]
        anyhow::bail!("No url or .rrd path given");
    };

    // Now what do we do with the data?
    if args.web_viewer {
        #[cfg(feature = "web")]
        {
            #[cfg(feature = "server")]
            if args.url_or_path.is_none() && args.port == re_ws_comms::DEFAULT_WS_SERVER_PORT {
                anyhow::bail!(
                    "Trying to spawn a websocket server on {}, but this port is \
                already used by the server we're connecting to. Please specify a different port.",
                    args.port
                );
            }

            // This is the server which the web viewer will talk to:
            re_log::info!("Starting a Rerun WebSocket Server…");
            let ws_server = re_ws_comms::Server::new(re_ws_comms::DEFAULT_WS_SERVER_PORT).await?;
            let server_handle = tokio::spawn(ws_server.listen(rx));

            let rerun_ws_server_url = re_ws_comms::default_server_url();
            host_web_viewer(rerun_ws_server_url).await?;

            return server_handle.await?;
        }

        #[cfg(not(feature = "web"))]
        anyhow::bail!("Can't host web-viewer - rerun was not compiled with the 'web' feature");
    } else {
        re_viewer::run_native_app(Box::new(move |cc, design_tokens| {
            let rx = re_viewer::wake_up_ui_thread_on_each_msg(rx, cc.egui_ctx.clone());
            let mut app =
                re_viewer::App::from_receiver(&cc.egui_ctx, design_tokens, cc.storage, rx);
            app.set_profiler(profiler);
            Box::new(app)
        }));
    }
    Ok(())
}

async fn connect_to_ws_url(
    args: &Args,
    profiler: re_viewer::Profiler,
    mut rerun_server_ws_url: String,
) -> anyhow::Result<()> {
    if !rerun_server_ws_url.contains("://") {
        rerun_server_ws_url = format!("{}://{rerun_server_ws_url}", re_ws_comms::PROTOCOL);
    }

    if args.web_viewer {
        host_web_viewer(rerun_server_ws_url).await?;
    } else {
        // By using RemoteViewerApp we let the user change the server they are connected to.
        re_viewer::run_native_app(Box::new(move |cc, design_tokens| {
            let mut app = re_viewer::RemoteViewerApp::new(
                &cc.egui_ctx,
                design_tokens,
                cc.storage,
                rerun_server_ws_url,
            );
            app.set_profiler(profiler);
            Box::new(app)
        }));
    }
    Ok(())
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
async fn host_web_viewer(rerun_ws_server_url: String) -> anyhow::Result<()> {
    let web_port = 9090;
    let viewer_url = format!("http://127.0.0.1:{}?url={}", web_port, rerun_ws_server_url);

    let web_server = re_web_server::WebServer::new(web_port);
    let web_server_handle = tokio::spawn(web_server.serve());

    let open = true;
    if open {
        webbrowser::open(&viewer_url).ok();
    } else {
        println!("Hosting Rerun Web Viewer at {viewer_url}.");
    }

    web_server_handle.await?
}

#[cfg(not(feature = "web"))]
async fn host_web_viewer(_rerun_ws_server_url: String) -> anyhow::Result<()> {
    panic!("Can't host web-viewer - rerun was not compiled with the 'web' feature");
}
