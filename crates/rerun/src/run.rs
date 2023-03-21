use std::sync::{atomic::AtomicBool, Arc};

use re_log_types::{LogMsg, PythonVersion};
use re_smart_channel::Receiver;

use anyhow::Context as _;
use clap::Subcommand;

#[cfg(feature = "web_viewer")]
use crate::web_viewer::host_web_viewer;

// Note the extra blank lines between the point-lists below: it is required by `clap`.

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
/// * `RERUN`: force enable/disable logging with rerun (only relevant for the Rerun API, not the Viewer itself). Either `on`/`1`/`true` or `off`/`0`/`false`
///
/// * `RERUN_SHADER_PATH`: change the search path for shader/shader-imports. WARNING: Shaders are embedded in some build configurations.
///
/// * `RERUN_TRACK_ALLOCATIONS`: track all allocations in order to find memory leaks in the viewer. WARNING: slows down the viewer by a lot!
///
/// * `WGPU_BACKEND`: overwrites the graphics backend used, must be one of `vulkan`, `metal`, `dx12`, `dx11`, or `gl`.
///     Naturally, support depends on your OS. Default is `vulkan` everywhere except on Mac where we use `metal`.
///
/// * `WGPU_POWER_PREF`: overwrites the power setting used for choosing a graphics adapter, must be `high` or `low`. (Default is `high`)
#[derive(Debug, clap::Parser)]
#[clap(author, about)]
struct Args {
    /// Print version and quit
    #[clap(long)]
    version: bool,

    /// Either a path to a `.rrd` file to load, an http url to an `.rrd` file,
    /// or a websocket url to a Rerun Server from which to read data
    ///
    /// If none is given, a server will be hosted which the Rerun SDK can connect to.
    url_or_path: Option<String>,

    /// What TCP port do we listen to (for SDK:s to connect to)?
    #[cfg(feature = "server")]
    #[clap(long, default_value_t = re_sdk_comms::DEFAULT_SERVER_PORT)]
    port: u16,

    /// Start the viewer in the browser (instead of locally).
    /// Requires Rerun to have been compiled with the 'web_viewer' feature.
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
    /// Configure the behavior of our analytics.
    #[cfg(all(feature = "analytics"))]
    #[command(subcommand)]
    Analytics(AnalyticsCommands),
}

#[derive(Debug, Clone, Subcommand)]
enum AnalyticsCommands {
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

/// Where are we calling [`run`] from?
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallSource {
    /// Called from a command-line-input (the terminal).
    Cli,

    /// Called from the Rerun Python SDK.
    Python(PythonVersion),
}

#[cfg(feature = "native_viewer")]
impl CallSource {
    fn is_python(&self) -> bool {
        matches!(self, Self::Python(_))
    }

    fn app_env(&self) -> re_viewer::AppEnvironment {
        match self {
            CallSource::Cli => re_viewer::AppEnvironment::RerunCli {
                rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
                llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
            },
            CallSource::Python(python_version) => {
                re_viewer::AppEnvironment::PythonSdk(python_version.clone())
            }
        }
    }
}

/// Run the Rerun application and return an exit code.
///
/// This is used by the `rerun` binary and the Rerun Python SDK via `python -m rerun [args...]`.
///
/// This installs crash panic and signal handlers that sends analytics on panics and signals.
/// These crash reports includes a stacktrace. We make sure the file paths in the stacktrace
/// don't include and sensitive parts of the path (like user names), but the function names
/// are all included, which means you should ONLY call `run` from a function with
/// a non-sensitive name.
///
/// In the future we plan to support installing user plugins (that act like callbacks),
/// and when we do we must make sure to give users an easy way to opt-out of the
/// crash callstacks, as those could include the file and function names of user code.
//
// It would be nice to use [`std::process::ExitCode`] here but
// then there's no good way to get back at the exit code from python
pub async fn run<I, T>(
    build_info: re_build_info::BuildInfo,
    call_source: CallSource,
    args: I,
) -> anyhow::Result<u8>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    #[cfg(feature = "native_viewer")]
    re_memory::accounting_allocator::turn_on_tracking_if_env_var(
        re_viewer::env_vars::RERUN_TRACK_ALLOCATIONS,
    );

    crate::crash_handler::install_crash_handlers(build_info);

    use clap::Parser as _;
    let args = Args::parse_from(args);

    if args.version {
        println!("{build_info}");
        return Ok(0);
    }

    let res = if let Some(commands) = &args.commands {
        match commands {
            #[cfg(all(feature = "analytics"))]
            Commands::Analytics(analytics) => run_analytics(analytics).map_err(Into::into),
            #[cfg(not(all(feature = "analytics")))]
            _ => Ok(()),
        }
    } else {
        run_impl(build_info, call_source, args).await
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

#[cfg(feature = "native_viewer")]
fn profiler(args: &Args) -> re_viewer::Profiler {
    let mut profiler = re_viewer::Profiler::default();
    if args.profile {
        profiler.start();
    }
    profiler
}

async fn run_impl(
    _build_info: re_build_info::BuildInfo,
    call_source: CallSource,
    args: Args,
) -> anyhow::Result<()> {
    #[cfg(feature = "native_viewer")]
    let profiler = profiler(&args);

    #[cfg(feature = "native_viewer")]
    let startup_options = re_viewer::StartupOptions {
        memory_limit: args.memory_limit.as_ref().map_or(Default::default(), |l| {
            re_memory::MemoryLimit::parse(l)
                .unwrap_or_else(|err| panic!("Bad --memory-limit: {err}"))
        }),
    };

    let (shutdown_rx, shutdown_bool) = setup_ctrl_c_handler();

    // Where do we get the data from?
    let rx = if let Some(url_or_path) = args.url_or_path.clone() {
        match categorize_argument(url_or_path) {
            ArgumentCategory::RrdHttpUrl(url) => {
                re_viewer::stream_rrd_from_http::stream_rrd_from_http_to_channel(url)
            }
            ArgumentCategory::RrdFilePath(path) => {
                re_log::info!("Loading {path:?}…");
                load_file_to_channel(&path).with_context(|| format!("{path:?}"))?
            }
            ArgumentCategory::WebSocketAddr(rerun_server_ws_url) => {
                // We are connecting to a server at a websocket address:

                if args.web_viewer {
                    #[cfg(feature = "web_viewer")]
                    {
                        let web_viewer =
                            host_web_viewer(true, rerun_server_ws_url, shutdown_rx.resubscribe());
                        return web_viewer.await;
                    }
                    #[cfg(not(feature = "web_viewer"))]
                    {
                        panic!("Can't host web-viewer - rerun was not compiled with the 'web_viewer' feature");
                    }
                } else {
                    #[cfg(feature = "native_viewer")]
                    return native_viewer_connect_to_ws_url(
                        _build_info,
                        call_source.app_env(),
                        startup_options,
                        profiler,
                        rerun_server_ws_url,
                    );

                    #[cfg(not(feature = "native_viewer"))]
                    {
                        _ = call_source;
                        anyhow::bail!("Can't start viewer - rerun was compiled without the 'native_viewer' feature");
                    }
                }
            }
        }
    } else {
        #[cfg(feature = "server")]
        {
            let server_options = re_sdk_comms::ServerOptions {
                max_latency_sec: parse_max_latency(args.drop_at_latency.as_ref()),

                // `rerun.spawn()` doesn't need to log that a connection has been made
                quiet: call_source.is_python(),
            };
            re_sdk_comms::serve(args.port, server_options, shutdown_rx.resubscribe())?
        }

        #[cfg(not(feature = "server"))]
        anyhow::bail!("No url or .rrd path given");
    };

    // Now what do we do with the data?

    if args.web_viewer {
        #[cfg(feature = "web_viewer")]
        {
            #[cfg(feature = "server")]
            if args.url_or_path.is_none() && args.port == re_ws_comms::DEFAULT_WS_SERVER_PORT {
                anyhow::bail!(
                    "Trying to spawn a websocket server on {}, but this port is \
                already used by the server we're connecting to. Please specify a different port.",
                    args.port
                );
            }

            // Make it possible to gracefully shutdown the servers on ctrl-c.
            let shutdown_ws_server = shutdown_rx.resubscribe();
            let shutdown_web_viewer = shutdown_rx.resubscribe();

            // This is the server which the web viewer will talk to:
            let ws_server = re_ws_comms::Server::new(re_ws_comms::DEFAULT_WS_SERVER_PORT).await?;
            let server_handle = tokio::spawn(ws_server.listen(rx, shutdown_ws_server));

            // This is the server that serves the Wasm+HTML:
            let ws_server_url = re_ws_comms::default_server_url();
            let ws_server_handle =
                tokio::spawn(host_web_viewer(true, ws_server_url, shutdown_web_viewer));

            // Wait for both servers to shutdown.
            ws_server_handle.await?.ok();
            return server_handle.await?;
        }

        #[cfg(not(feature = "web_viewer"))]
        {
            _ = (call_source, rx);
            anyhow::bail!(
                "Can't host web-viewer - rerun was not compiled with the 'web_viewer' feature"
            );
        }
    } else {
        #[cfg(feature = "native_viewer")]
        return re_viewer::run_native_app(Box::new(move |cc, re_ui| {
            // We need to wake up the ui thread in order to process shutdown signals.
            let ctx = cc.egui_ctx.clone();
            let mut shutdown_repaint = shutdown_rx.resubscribe();
            tokio::spawn(async move {
                shutdown_repaint.recv().await.unwrap();
                ctx.request_repaint();
            });

            let rx = re_viewer::wake_up_ui_thread_on_each_msg(rx, cc.egui_ctx.clone());
            let mut app = re_viewer::App::from_receiver(
                _build_info,
                &call_source.app_env(),
                startup_options,
                re_ui,
                cc.storage,
                rx,
                shutdown_bool,
            );
            app.set_profiler(profiler);
            Box::new(app)
        }))
        .map_err(|e| e.into());

        #[cfg(not(feature = "native_viewer"))]
        {
            _ = (call_source, rx);
            anyhow::bail!(
                "Can't start viewer - rerun was compiled without the 'native_viewer' feature"
            );
        }
    }
}

enum ArgumentCategory {
    /// A remote RRD file, served over http.
    RrdHttpUrl(String),

    /// A path to a local file.
    RrdFilePath(std::path::PathBuf),

    /// A remote Rerun server.
    WebSocketAddr(String),
}

fn categorize_argument(mut uri: String) -> ArgumentCategory {
    let path = std::path::Path::new(&uri).to_path_buf();

    if uri.starts_with("http") {
        ArgumentCategory::RrdHttpUrl(uri)
    } else if uri.starts_with("ws") {
        ArgumentCategory::WebSocketAddr(uri)
    } else if uri.starts_with("file://") || path.exists() || uri.ends_with(".rrd") {
        ArgumentCategory::RrdFilePath(path)
    } else {
        // If this is sometyhing like `foo.com` we can't know what it is until we connect to it.
        // We could/should connect and see what it is, but for now we just take a wild guess instead:
        re_log::debug!("Assuming WebSocket endpoint");
        if !uri.contains("://") {
            uri = format!("{}://{uri}", re_ws_comms::PROTOCOL);
        }
        ArgumentCategory::WebSocketAddr(uri)
    }
}

#[cfg(feature = "native_viewer")]
fn native_viewer_connect_to_ws_url(
    build_info: re_build_info::BuildInfo,
    app_env: re_viewer::AppEnvironment,
    startup_options: re_viewer::StartupOptions,
    profiler: re_viewer::Profiler,
    rerun_server_ws_url: String,
) -> anyhow::Result<()> {
    // By using RemoteViewerApp we let the user change the server they are connected to.
    re_viewer::run_native_app(Box::new(move |cc, re_ui| {
        let mut app = re_viewer::RemoteViewerApp::new(
            build_info,
            app_env,
            startup_options,
            re_ui,
            cc.storage,
            rerun_server_ws_url,
        );
        app.set_profiler(profiler);
        Box::new(app)
    }))?;
    Ok(())
}

fn load_file_to_channel(path: &std::path::Path) -> anyhow::Result<Receiver<LogMsg>> {
    use anyhow::Context as _;
    let file = std::fs::File::open(path).context("Failed to open file")?;
    let decoder = re_log_types::encoding::Decoder::new(file)?;

    let (tx, rx) = re_smart_channel::smart_channel(re_smart_channel::Source::File {
        path: path.to_owned(),
    });

    let path = path.to_owned();
    std::thread::Builder::new()
        .name("rrd_file_reader".into())
        .spawn(move || {
            for msg in decoder {
                match msg {
                    Ok(msg) => {
                        tx.send(msg).ok();
                    }
                    Err(err) => {
                        re_log::warn_once!("Failed to decode message in {path:?}: {err}");
                    }
                }
            }
        })
        .expect("Failed to spawn thread");

    Ok(rx)
}

#[cfg(feature = "server")]
fn parse_max_latency(max_latency: Option<&String>) -> f32 {
    max_latency.as_ref().map_or(f32::INFINITY, |time| {
        re_format::parse_duration(time)
            .unwrap_or_else(|err| panic!("Failed to parse max_latency ({max_latency:?}): {err}"))
    })
}

pub fn setup_ctrl_c_handler() -> (tokio::sync::broadcast::Receiver<()>, Arc<AtomicBool>) {
    let (sender, receiver) = tokio::sync::broadcast::channel(1);
    let shutdown_return = Arc::new(AtomicBool::new(false));
    let shutdown = shutdown_return.clone();
    ctrlc::set_handler(move || {
        re_log::debug!("Ctrl-C detected, shutting down.");
        sender.send(()).unwrap();
        shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");
    (receiver, shutdown_return)
}
