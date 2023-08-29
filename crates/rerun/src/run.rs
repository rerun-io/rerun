use std::path::{Path, PathBuf};

use anyhow::Context as _;
use clap::Subcommand;
use itertools::Itertools;
use rayon::prelude::{IntoParallelIterator, ParallelIterator};

use re_data_source::DataSource;
use re_log_types::{LogMsg, PythonVersion};
use re_smart_channel::{ReceiveSet, Receiver, SmartMessagePayload};

#[cfg(feature = "web_viewer")]
use re_sdk::web_viewer::host_web_viewer;
#[cfg(feature = "web_viewer")]
use re_web_viewer_server::WebViewerServerPort;
#[cfg(feature = "web_viewer")]
use re_ws_comms::RerunServerPort;

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
    // Note: arguments are sorted lexicographically for nicer `--help` message:
    #[command(subcommand)]
    commands: Option<Commands>,

    /// What bind address IP to use.
    #[clap(long, default_value = "0.0.0.0")]
    bind: String,

    /// Set a maximum input latency, e.g. "200ms" or "10s".
    ///
    /// If we go over this, we start dropping packets.
    ///
    /// The default is no limit, which means Rerun might eat more and more memory,
    /// and have longer and longer latency, if you are logging data faster
    /// than Rerun can index it.
    #[clap(long)]
    drop_at_latency: Option<String>,

    /// An upper limit on how much memory the Rerun Viewer should use.
    ///
    /// When this limit is used, Rerun will purge the oldest data.
    ///
    /// Example: `16GB`
    #[clap(long)]
    memory_limit: Option<String>,

    /// Whether the Rerun Viewer should persist the state of the viewer to disk.
    ///
    /// When persisted, the state will be stored at the following locations:
    ///
    /// - Linux: /home/UserName/.local/share/rerun
    ///
    /// - macOS: /Users/UserName/Library/Application Support/rerun
    ///
    /// - Windows: C:\Users\UserName\AppData\Roaming\rerun
    #[clap(long, default_value_t = true)]
    persist_state: bool,

    /// What TCP port do we listen to (for SDKs to connect to)?
    #[cfg(feature = "server")]
    #[clap(long, default_value_t = re_sdk_comms::DEFAULT_SERVER_PORT)]
    port: u16,

    /// Start with the puffin profiler running.
    #[clap(long)]
    profile: bool,

    /// Stream incoming log events to an .rrd file at the given path.
    #[clap(long)]
    save: Option<String>,

    /// Take a screenshot of the app and quit.
    /// We use this to generate screenshots of our exmples.
    /// Useful together with `--window-size`.
    #[clap(long)]
    screenshot_to: Option<std::path::PathBuf>,

    /// Do not display the welcome screen.
    #[clap(long)]
    skip_welcome_screen: bool,

    /// Exit with a non-zero exit code if any warning or error is logged. Useful for tests.
    #[clap(long)]
    strict: bool,

    /// Ingest data and then quit once the goodbye message has been received.
    ///
    /// Used for testing together with the `--strict` argument.
    ///
    /// Fails if no messages are received, or if no messages are received within a dozen or so seconds.
    #[clap(long)]
    test_receive: bool,

    /// Either: a path to `.rrd` file(s) to load,
    /// some mesh or image files to show,
    /// an http url to an `.rrd` file,
    /// or a websocket url to a Rerun Server from which to read data
    ///
    /// If none is given, a server will be hosted which the Rerun SDK can connect to.
    url_or_paths: Vec<String>,

    /// Print version and quit
    #[clap(long)]
    version: bool,

    /// Start the viewer in the browser (instead of locally).
    /// Requires Rerun to have been compiled with the 'web_viewer' feature.
    #[clap(long)]
    web_viewer: bool,

    /// What port do we listen to for hosting the web viewer over HTTP.
    /// A port of 0 will pick a random port.
    #[cfg(feature = "web_viewer")]
    #[clap(long, default_value_t = Default::default())]
    web_viewer_port: WebViewerServerPort,

    /// Set the screen resolution (in logical points), e.g. "1920x1080".
    /// Useful together with `--screenshot-to`.
    #[clap(long)]
    window_size: Option<String>,

    /// What port do we listen to for incoming websocket connections from the viewer
    /// A port of 0 will pick a random port.
    #[cfg(feature = "web_viewer")]
    #[clap(long, default_value_t = Default::default())]
    ws_server_port: RerunServerPort,
}

#[derive(Debug, Clone, Subcommand)]
enum Commands {
    /// Configure the behavior of our analytics.
    #[cfg(feature = "analytics")]
    #[command(subcommand)]
    Analytics(AnalyticsCommands),

    /// Compares the data between 2 .rrd files, returning a successful shell exit code if they
    /// match.
    ///
    /// This ignores the `log_time` timeline.
    Compare {
        path_to_rrd1: String,
        path_to_rrd2: String,

        /// If specified, dumps both .rrd files as tables.
        #[clap(long, default_value_t = false)]
        full_dump: bool,
    },
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

impl CallSource {
    #[allow(dead_code)]
    fn is_python(&self) -> bool {
        matches!(self, Self::Python(_))
    }

    #[cfg(feature = "native_viewer")]
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
/// This is used by the `rerun` binary and the Rerun Python SDK via `python -m rerun [args…]`.
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

    re_crash_handler::install_crash_handlers(build_info);

    use clap::Parser as _;
    let args = Args::parse_from(args);

    if args.version {
        println!("{build_info}");
        return Ok(0);
    }

    if args.strict {
        re_log::add_boxed_logger(Box::new(StrictLogger {})).expect("Failed to enter --strict mode");
        re_log::info!("--strict mode: any warning or error will cause Rerun to panic.");
    }

    let res = if let Some(commands) = &args.commands {
        match commands {
            #[cfg(feature = "analytics")]
            Commands::Analytics(analytics) => run_analytics(analytics).map_err(Into::into),

            Commands::Compare {
                path_to_rrd1,
                path_to_rrd2,
                full_dump,
            } => {
                let path_to_rrd1 = PathBuf::from(path_to_rrd1);
                let path_to_rrd2 = PathBuf::from(path_to_rrd2);
                run_compare(&path_to_rrd1, &path_to_rrd2, *full_dump)
            }
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
            re_log::warn!("{err}");
            Ok(1)
        }
        // Unclean failure -- re-raise exception
        Err(err) => Err(err),
    }
}

/// Checks whether two .rrd files are _similar_, i.e. not equal on a byte-level but
/// functionally equivalent.
///
/// Returns `Ok(())` if they match, or an error containing a detailed diff otherwise.
fn run_compare(path_to_rrd1: &Path, path_to_rrd2: &Path, full_dump: bool) -> anyhow::Result<()> {
    /// Given a path to an rrd file, builds up a `DataStore` and returns its contents as one big
    /// `DataTable`.
    ///
    /// Fails if there are more than one data recordings present in the rrd file.
    fn compute_uber_table(path_to_rrd: &Path) -> anyhow::Result<re_log_types::DataTable> {
        use re_data_store::StoreDb;
        use re_log_types::StoreId;

        let rrd_file = std::fs::File::open(path_to_rrd)
            .with_context(|| format!("couldn't open rrd file contents at {path_to_rrd:?}"))?;

        let mut stores: std::collections::HashMap<StoreId, StoreDb> = Default::default();
        let decoder = re_log_encoding::decoder::Decoder::new(rrd_file)?;
        for msg in decoder {
            let msg = msg
                .with_context(|| format!("couldn't decode rrd file contents at {path_to_rrd:?}"))?;
            stores
                .entry(msg.store_id().clone())
                .or_insert(re_data_store::StoreDb::new(msg.store_id().clone()))
                .add(&msg)
                .with_context(|| format!("couldn't decode rrd file contents at {path_to_rrd:?}"))?;
        }

        let mut stores = stores
            .values()
            .filter(|store| store.store_kind() == re_log_types::StoreKind::Recording)
            .collect_vec();

        anyhow::ensure!(
            !stores.is_empty(),
            "no data recording found in rrd file at {path_to_rrd:?}"
        );
        anyhow::ensure!(
            stores.len() == 1,
            "more than one data recording found in rrd file at {path_to_rrd:?}"
        );

        let store = stores.pop().unwrap(); // safe, ensured above

        Ok::<_, anyhow::Error>(store.store().to_data_table())
    }

    let table1 = compute_uber_table(path_to_rrd1)?;
    let table2 = compute_uber_table(path_to_rrd2)?;

    if full_dump {
        println!("{table1}");
        println!("{table2}");
    }

    re_log_types::DataTable::similar(&table1, &table2)
}

#[cfg(feature = "analytics")]
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
fn profiler(args: &Args) -> re_tracing::Profiler {
    let mut profiler = re_tracing::Profiler::default();
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
        persist_state: args.persist_state,
        screenshot_to_path_then_quit: args.screenshot_to.clone(),

        skip_welcome_screen: args.skip_welcome_screen,

        // TODO(emilk): make it easy to set this on eframe instead
        resolution_in_points: if let Some(size) = &args.window_size {
            Some(parse_size(size)?)
        } else {
            None
        },
    };

    // Where do we get the data from?
    let rx: Vec<Receiver<LogMsg>> = if args.url_or_paths.is_empty() {
        #[cfg(feature = "server")]
        {
            let server_options = re_sdk_comms::ServerOptions {
                max_latency_sec: parse_max_latency(args.drop_at_latency.as_ref()),

                // `rerun.spawn()` doesn't need to log that a connection has been made
                quiet: call_source.is_python(),
            };
            let rx = re_sdk_comms::serve(&args.bind, args.port, server_options).await?;
            vec![rx]
        }

        #[cfg(not(feature = "server"))]
        vec![]
    } else {
        let data_sources = args
            .url_or_paths
            .iter()
            .cloned()
            .map(DataSource::from_uri)
            .collect_vec();

        #[cfg(feature = "web_viewer")]
        if data_sources.len() == 1 && args.web_viewer {
            if let DataSource::WebSocketAddr(rerun_server_ws_url) = data_sources[0].clone() {
                // Special case! We are connecting a web-viewer to a web-socket address.
                // Instead of piping, just host a web-viewer that connects to the web-socket directly:
                return host_web_viewer(
                    args.bind.clone(),
                    args.web_viewer_port,
                    true,
                    rerun_server_ws_url,
                )
                .await;
            }
        }

        data_sources
            .into_par_iter()
            .map(|data_source| data_source.stream(None))
            .collect::<Result<Vec<_>, _>>()?
    };

    // Now what do we do with the data?

    if args.test_receive {
        let rx = ReceiveSet::new(rx);
        assert_receive_into_store_db(&rx).map(|_db| ())
    } else if let Some(rrd_path) = args.save {
        let rx = ReceiveSet::new(rx);
        Ok(stream_to_rrd_on_disk(&rx, &rrd_path.into())?)
    } else if args.web_viewer {
        #[cfg(feature = "web_viewer")]
        {
            #[cfg(feature = "server")]
            if args.url_or_paths.is_empty()
                && (args.port == args.web_viewer_port.0 || args.port == args.ws_server_port.0)
            {
                anyhow::bail!(
                    "Trying to spawn a websocket server on {}, but this port is \
                already used by the server we're connecting to. Please specify a different port.",
                    args.port
                );
            }

            // This is the server which the web viewer will talk to:
            let ws_server =
                re_ws_comms::RerunServer::new(args.bind.clone(), args.ws_server_port).await?;
            let ws_server_url = ws_server.server_url();
            let rx = ReceiveSet::new(rx);
            let ws_server_handle = tokio::spawn(ws_server.listen(rx));

            // This is the server that serves the Wasm+HTML:
            let web_server_handle = tokio::spawn(host_web_viewer(
                args.bind.clone(),
                args.web_viewer_port,
                true,
                ws_server_url,
            ));

            // Wait for both servers to shutdown.
            web_server_handle.await?.ok();
            return ws_server_handle.await?.map_err(anyhow::Error::from);
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
            let mut app = re_viewer::App::new(
                _build_info,
                &call_source.app_env(),
                startup_options,
                re_ui,
                cc.storage,
            );
            for rx in rx {
                app.add_receiver(rx);
            }
            app.set_profiler(profiler);
            Box::new(app)
        }))
        .map_err(|err| err.into());

        #[cfg(not(feature = "native_viewer"))]
        {
            _ = (call_source, rx);
            anyhow::bail!(
                "Can't start viewer - rerun was compiled without the 'native_viewer' feature"
            );
        }
    }
}

#[cfg(feature = "native_viewer")]
fn parse_size(size: &str) -> anyhow::Result<[f32; 2]> {
    fn parse_size_inner(size: &str) -> Option<[f32; 2]> {
        let (w, h) = size.split_once('x')?;
        let w = w.parse().ok()?;
        let h = h.parse().ok()?;
        Some([w, h])
    }

    parse_size_inner(size)
        .ok_or_else(|| anyhow::anyhow!("Invalid size {:?}, expected e.g. 800x600", size))
}

// NOTE: This is only used as part of end-to-end tests.
fn assert_receive_into_store_db(rx: &ReceiveSet<LogMsg>) -> anyhow::Result<re_data_store::StoreDb> {
    re_log::info!("Receiving messages into a StoreDb…");

    let mut db: Option<re_data_store::StoreDb> = None;

    let mut num_messages = 0;

    let timeout = std::time::Duration::from_secs(12);

    loop {
        if !rx.is_connected() {
            anyhow::bail!("Channel disconnected without a Goodbye message.");
        }

        match rx.recv_timeout(timeout) {
            Some((_, msg)) => {
                re_log::info_once!("Received first message.");

                match msg.payload {
                    SmartMessagePayload::Msg(msg) => {
                        let mut_db = db.get_or_insert_with(|| {
                            re_data_store::StoreDb::new(msg.store_id().clone())
                        });

                        mut_db.add(&msg)?;
                        num_messages += 1;
                    }
                    SmartMessagePayload::Quit(err) => {
                        if let Some(err) = err {
                            anyhow::bail!("data source has disconnected unexpectedly: {err}")
                        } else if let Some(db) = db {
                            db.entity_db.data_store.sanity_check()?;
                            anyhow::ensure!(0 < num_messages, "No messages received");
                            re_log::info!("Successfully ingested {num_messages} messages.");
                            return Ok(db);
                        } else {
                            anyhow::bail!("StoreDb never initialized");
                        }
                    }
                }
            }
            None => {
                anyhow::bail!(
                    "Didn't receive any messages within {} seconds. Giving up.",
                    timeout.as_secs()
                );
            }
        }
    }
}

fn stream_to_rrd_on_disk(
    rx: &re_smart_channel::ReceiveSet<LogMsg>,
    path: &std::path::PathBuf,
) -> Result<(), re_log_encoding::FileSinkError> {
    use re_log_encoding::FileSinkError;
    use re_smart_channel::RecvError;

    if path.exists() {
        re_log::warn!("Overwriting existing file at {path:?}");
    }

    re_log::info!("Saving incoming log stream to {path:?}. Abort with Ctrl-C.");

    let encoding_options = re_log_encoding::EncodingOptions::COMPRESSED;
    let file =
        std::fs::File::create(path).map_err(|err| FileSinkError::CreateFile(path.clone(), err))?;
    let mut encoder = re_log_encoding::encoder::Encoder::new(encoding_options, file)?;

    loop {
        match rx.recv() {
            Ok(msg) => {
                if let Some(payload) = msg.into_data() {
                    encoder.append(&payload)?;
                }
            }
            Err(RecvError) => {
                re_log::info!("Log stream disconnected, stopping.");
                break;
            }
        }
    }

    re_log::info!("File saved to {path:?}");

    Ok(())
}

#[cfg(feature = "server")]
fn parse_max_latency(max_latency: Option<&String>) -> f32 {
    max_latency.as_ref().map_or(f32::INFINITY, |time| {
        re_format::parse_duration(time)
            .unwrap_or_else(|err| panic!("Failed to parse max_latency ({max_latency:?}): {err}"))
    })
}

// ----------------------------------------------------------------------------

use re_log::external::log;

struct StrictLogger {}

impl log::Log for StrictLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        match metadata.level() {
            log::Level::Error | log::Level::Warn => true,
            log::Level::Info | log::Level::Debug | log::Level::Trace => false,
        }
    }

    fn log(&self, record: &log::Record<'_>) {
        let level = match record.level() {
            log::Level::Error => "error",
            log::Level::Warn => "warning",
            log::Level::Info | log::Level::Debug | log::Level::Trace => return,
        };

        eprintln!("{level} logged in --strict mode: {}", record.args());
        eprintln!(
            "{}",
            re_crash_handler::callstack_from(&["log::__private_api_log"])
        );
        std::process::exit(1);
    }

    fn flush(&self) {}
}
