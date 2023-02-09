//! The main Rerun binary.
//!
//! This can act either as a server, a viewer, or both, depending on which options you use when you start it.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
//!

// NOTE: Have a look at `re_sdk/src/lib.rs` for an accurate listing of all these symbols.
#[cfg(feature = "sdk")]
pub use re_sdk::*;

#[cfg(not(feature = "sdk"))]
use re_log_types::LogMsg;

// ---

use anyhow::Context as _;

use clap::Subcommand;
use re_format::parse_duration;
use re_smart_channel::Receiver;

/// The Rerun Viewer and Server
///
/// Features:
///
/// * Read `.rrd` (rerun recording files).
///
/// * Connect to a Rerun Server over web-sockets.
///
/// * Host a Rerun Server that Rerun SDK:s can connect to.
///
/// Environment variables:
///
/// * `RERUN_TRACK_ALLOCATIONS`: track all allocations in order to find memory leaks in the viewer. WARNING: slows down the viewer by a lot!
#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
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

    /// An upper limit on how much memory the Rerun Viewer should use.
    ///
    /// When this limit is used, Rerun will purge the oldest data.
    ///
    /// Example: `16GB`
    #[clap(long)]
    memory_limit: Option<String>,

    /// Set a maximum input latency, e.g. "200ms" or "10s".
    ///
    /// If we go over this, we start dropping packets.
    ///
    /// The default is no limit, which means Rerun might eat more and more memory,
    /// and have longer and longer latency, if you are logging data faster
    /// than Rerun can index it.
    #[clap(long)]
    drop_at_latency: Option<String>,

    #[command(subcommand)]
    commands: Option<Commands>,
}

#[derive(Debug, Clone, Subcommand)]
enum Commands {
    /// Configure the behaviour of our analytics.
    #[cfg(all(feature = "analytics"))]
    #[command(subcommand)]
    Analytics(AnalyticsCommands),
}

#[derive(Debug, Clone, Subcommand)]
pub enum AnalyticsCommands {
    /// Prints extra information about analytics.
    Details,

    /// Deletes everything related to analytics.
    ///
    /// This will remove all pending data that hasn't yet been sent to our servers, as well as
    /// reset your analytics ID.
    Clear,

    /// Associate an email address with the current user.
    Email { email: String },

    /// Enable analytics.
    Enable,

    /// Disable analytics.
    Disable,

    /// Prints the current configuration.
    Config,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallSource {
    Cli,

    Python,
}

// Run the rerun application and return an exit code
// If we be nice to use `std::process::ExitCode` here but
// then there's no good way to get back at the exit code from
// python
pub async fn run<I, T>(call_source: CallSource, args: I) -> anyhow::Result<u8>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    re_memory::accounting_allocator::turn_on_tracking_if_env_var(
        re_viewer::env_vars::RERUN_TRACK_ALLOCATIONS,
    );

    use clap::Parser as _;
    let args = Args::parse_from(args);

    let res = if let Some(commands) = &args.commands {
        match commands {
            #[cfg(all(feature = "analytics"))]
            Commands::Analytics(analytics) => run_analytics(analytics).map_err(Into::into),
            #[cfg(not(all(feature = "analytics")))]
            _ => Ok(()),
        }
    } else {
        run_impl(call_source, args).await
    };

    match res {
        // Clean success
        Ok(_) => Ok(0),
        // Clean failure -- known error AddrInUse
        Err(err)
            if err
                .downcast_ref::<std::io::Error>()
                .map_or(false, |io_err| {
                    io_err.kind() == std::io::ErrorKind::AddrInUse
                }) =>
        {
            re_log::warn!("{err}. Another Rerun instance is probably running.");
            Ok(1)
        }
        // Unclean failure -- re-raise exception
        Err(err) => Err(err),
    }
}

#[cfg(all(feature = "analytics"))]
fn run_analytics(cmd: &AnalyticsCommands) -> Result<(), re_analytics::cli::CliError> {
    match cmd {
        #[allow(clippy::unit_arg)]
        AnalyticsCommands::Details => Ok(re_analytics::cli::print_details()),
        AnalyticsCommands::Clear => re_analytics::cli::clear(),
        AnalyticsCommands::Email { email } => {
            re_analytics::cli::set([("email".to_owned(), email.clone().into())])
        }
        AnalyticsCommands::Enable => re_analytics::cli::opt(true),
        AnalyticsCommands::Disable => re_analytics::cli::opt(false),
        AnalyticsCommands::Config => re_analytics::cli::print_config(),
    }
}

async fn run_impl(call_source: CallSource, args: Args) -> anyhow::Result<()> {
    let mut profiler = re_viewer::Profiler::default();
    if args.profile {
        profiler.start();
    }

    let startup_options = re_viewer::StartupOptions {
        memory_limit: args.memory_limit.as_ref().map_or(Default::default(), |l| {
            re_memory::MemoryLimit::parse(l)
                .unwrap_or_else(|err| panic!("Bad --memory-limit: {err}"))
        }),
    };

    // Where do we get the data from?
    let rx = if let Some(url_or_path) = &args.url_or_path {
        let path = std::path::Path::new(url_or_path).to_path_buf();
        if path.exists() || url_or_path.ends_with(".rrd") {
            re_log::info!("Loading {path:?}…");
            load_file_to_channel(&path).with_context(|| format!("{path:?}"))?
        } else {
            // We are connecting to a server at a websocket address:
            return connect_to_ws_url(&args, startup_options, profiler, url_or_path.clone()).await;
        }
    } else {
        #[cfg(feature = "server")]
        {
            let server_options = re_sdk_comms::ServerOptions {
                max_latency_sec: parse_max_latency(args.drop_at_latency.as_ref()),

                // `rerun.spawn()` doesn't ned to log that a connection has been made
                quiet: call_source == CallSource::Python,
            };
            re_sdk_comms::serve(args.port, server_options)?
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
            let ws_server = re_ws_comms::Server::new(re_ws_comms::DEFAULT_WS_SERVER_PORT).await?;
            let server_handle = tokio::spawn(ws_server.listen(rx));

            let rerun_ws_server_url = re_ws_comms::default_server_url();
            host_web_viewer(rerun_ws_server_url).await?;

            return server_handle.await?;
        }

        #[cfg(not(feature = "web"))]
        anyhow::bail!("Can't host web-viewer - rerun was not compiled with the 'web' feature");
    } else {
        re_viewer::run_native_app(Box::new(move |cc, re_ui| {
            let rx = wake_up_ui_thread_on_each_msg(rx, cc.egui_ctx.clone());
            let mut app = re_viewer::App::from_receiver(startup_options, re_ui, cc.storage, rx);
            app.set_profiler(profiler);
            Box::new(app)
        }))?;
    }
    Ok(())
}

async fn connect_to_ws_url(
    args: &Args,
    startup_options: re_viewer::StartupOptions,
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
        re_viewer::run_native_app(Box::new(move |cc, re_ui| {
            let mut app = re_viewer::RemoteViewerApp::new(
                startup_options,
                re_ui,
                cc.storage,
                rerun_server_ws_url,
            );
            app.set_profiler(profiler);
            Box::new(app)
        }))?;
    }
    Ok(())
}

fn load_file_to_channel(path: &std::path::Path) -> anyhow::Result<Receiver<LogMsg>> {
    use anyhow::Context as _;
    let file = std::fs::File::open(path).context("Failed to open file")?;
    let decoder = re_log_types::encoding::Decoder::new(file)?;

    let (tx, rx) = re_smart_channel::smart_channel(re_smart_channel::Source::File {
        path: path.to_owned(),
    });

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
    let viewer_url = format!("http://127.0.0.1:{web_port}?url={rerun_ws_server_url}");

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

/// This wakes up the ui thread each time we receive a new message.
fn wake_up_ui_thread_on_each_msg<T: Send + 'static>(
    rx: Receiver<T>,
    ctx: egui::Context,
) -> re_smart_channel::Receiver<T> {
    // We need to intercept messages to wake up the ui thread.
    // For that, we need a new channel.
    // However, we want to make sure the channel latency numbers are from the start
    // of the first channel, to the end of the second.
    // For that we need to use `chained_channel`, `recv_with_send_time` and `send_at`.
    let (tx, new_rx) = rx.chained_channel();
    std::thread::Builder::new()
        .name("ui_waker".to_owned())
        .spawn(move || {
            while let Ok((sent_at, msg)) = rx.recv_with_send_time() {
                if tx.send_at(sent_at, msg).is_ok() {
                    ctx.request_repaint();
                } else {
                    break;
                }
            }
            re_log::debug!("Shutting down ui_waker thread");
        })
        .unwrap();
    new_rx
}

#[cfg(feature = "server")]
fn parse_max_latency(max_latency: Option<&String>) -> f32 {
    max_latency.as_ref().map_or(f32::INFINITY, |time| {
        parse_duration(time)
            .unwrap_or_else(|err| panic!("Failed to parse max_latency ({max_latency:?}): {err}"))
    })
}
