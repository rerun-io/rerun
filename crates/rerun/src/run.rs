use std::path::{Path, PathBuf};

use anyhow::Context as _;
use clap::Subcommand;
use itertools::Itertools;

use re_data_source::DataSource;
use re_log_types::{DataTable, LogMsg, PythonVersion, SetStoreInfo};
use re_smart_channel::{ReceiveSet, Receiver, SmartMessagePayload};

#[cfg(feature = "web_viewer")]
use re_sdk::web_viewer::host_web_viewer;
use re_types::SizeBytes;
#[cfg(feature = "web_viewer")]
use re_web_viewer_server::WebViewerServerPort;
#[cfg(feature = "server")]
use re_ws_comms::RerunServerPort;

const SHORT_ABOUT: &str = "The Rerun Viewer and Server";

// Place the important help _last_, to make it most visible in the terminal.
const EXAMPLES: &str = r#"
Environment variables:
    RERUN_SHADER_PATH         The search path for shader/shader-imports. Only available in developer builds.
    RERUN_TRACK_ALLOCATIONS   Track memory allocations to diagnose memory leaks in the viewer. WARNING: slows down the viewer by a lot!
    RUST_LOG                  Change the log level of the viewer, e.g. `RUST_LOG=debug`.
    WGPU_BACKEND              Overwrites the graphics backend used, must be one of `vulkan`, `metal` or `gl`.
                              Default is `vulkan` everywhere except on Mac where we use `metal`. What is supported depends on your OS.
    WGPU_POWER_PREF           Overwrites the power setting used for choosing a graphics adapter, must be `high` or `low`. (Default is `high`)


Examples:
    Open a Rerun Viewer that listens for incoming SDK connections:
        rerun

    Load some files and show them in the Rerun Viewer:
        rerun recording.rrd mesh.obj image.png https://example.com/recording.rrd

    Open an .rrd file and stream it to a Web Viewer:
        rerun recording.rrd --web-viewer

    Host a Rerun Server which listens for incoming TCP connections from the logging SDK, buffer the log messages, and host the results over WebSocket:
        rerun --serve

    Host a Rerun Server which serves a recording over WebSocket to any connecting Rerun Viewers:
        rerun --serve recording.rrd

    Connect to a Rerun Server:
        rerun ws://localhost:9877

    Listen for incoming TCP connections from the logging SDK and stream the results to disk:
        rerun --save new_recording.rrd
"#;

#[derive(Debug, clap::Parser)]
#[clap(
    about = SHORT_ABOUT,
    // Place most of the help last, as that is most visible in the terminal.
    after_long_help = EXAMPLES
)]
struct Args {
    // Note: arguments are sorted lexicographically for nicer `--help` message.
    //
    // We also use `long_help` on some arguments for more compact formatting.
    //
    #[command(subcommand)]
    command: Option<Command>,

    /// What bind address IP to use.
    #[clap(long, default_value = "0.0.0.0")]
    bind: String,

    /// Set a maximum input latency, e.g. "200ms" or "10s".
    ///
    /// If we go over this, we start dropping packets.
    ///
    /// The default is no limit, which means Rerun might eat more and more memory
    /// and have longer and longer latency, if you are logging data faster
    /// than Rerun can index it.
    #[clap(long)]
    drop_at_latency: Option<String>,

    #[clap(
        long,
        default_value = "75%",
        long_help = r"An upper limit on how much memory the Rerun Viewer should use.
When this limit is reached, Rerun will drop the oldest data.
Example: `16GB` or `50%` (of system total)."
    )]
    memory_limit: String,

    #[clap(
        long,
        default_value = "25%",
        long_help = r"An upper limit on how much memory the WebSocket server should use.
The server buffers log messages for the benefit of late-arriving viewers.
When this limit is reached, Rerun will drop the oldest data.
Example: `16GB` or `50%` (of system total)."
    )]
    server_memory_limit: String,

    #[clap(
        long,
        default_value_t = true,
        long_help = r"Whether the Rerun Viewer should persist the state of the viewer to disk.
When persisted, the state will be stored at the following locations:
- Linux: /home/UserName/.local/share/rerun
- macOS: /Users/UserName/Library/Application Support/rerun
- Windows: C:\Users\UserName\AppData\Roaming\rerun"
    )]
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

    /// Serve the recordings over WebSocket to one or more Rerun Viewers.
    ///
    /// This will also host a web-viewer over HTTP that can connect to the WebSocket address,
    /// but you can also connect with the native binary.
    ///
    /// `rerun --serve` will act like a proxy,
    /// listening for incoming TCP connection from logging SDKs, and forwarding it to
    /// Rerun viewers.
    #[clap(long)]
    serve: bool,

    /// Do not display the welcome screen.
    #[clap(long)]
    skip_welcome_screen: bool,

    /// The number of compute threads to use.
    ///
    /// If zero, the same number of threads as the number of cores will be used.
    /// If negative, will use that much fewer threads than cores.
    ///
    /// Rerun will still use some additional threads for I/O.
    #[clap(
        long,
        short = 'j',
        default_value = "-2", // save some CPU for the main thread and the rest of the users system
    )]
    threads: i32,

    #[clap(long_help = r"Any combination of:
- A WebSocket url to a Rerun Server
- An HTTP(S) URL to an .rrd file to load
- A path to an rerun .rrd recording
- A path to an image or mesh, or any other file that Rerun can load (see https://www.rerun.io/docs/howto/open-any-file)

If no arguments are given, a server will be hosted which a Rerun SDK can connect to.")]
    url_or_paths: Vec<String>,

    /// Print version and quit
    #[clap(long)]
    version: bool,

    /// Start the viewer in the browser (instead of locally).
    ///
    /// Requires Rerun to have been compiled with the 'web_viewer' feature.
    ///
    /// This implies `--serve`.
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
    #[cfg(feature = "server")]
    #[clap(long, default_value_t = Default::default())]
    ws_server_port: RerunServerPort,

    /// Override the default graphics backend and for a specific one instead.
    ///
    /// When using `--web-viewer` this should be one of:
    /// * `webgpu`
    /// * `webgl`
    ///
    /// When starting a native viewer instead this should be one of:
    /// * `vulkan` (Linux & Windows only)
    /// * `gl` (Linux & Windows only)
    /// * `metal` (macOS only)
    // Note that we don't compile with DX12 right now, but we could (we don't since this adds permutation and wgpu still has some issues with it).
    // GL could be enabled on MacOS via `angle` but given prior issues with ANGLE this seems to be a bad idea!
    #[clap(long)]
    renderer: Option<String>,

    // ----------------------------------------------------------------------------
    // Debug-options:
    /// Ingest data and then quit once the goodbye message has been received.
    ///
    /// Used for testing together with `RERUN_PANIC_ON_WARN=1`.
    ///
    /// Fails if no messages are received, or if no messages are received within a dozen or so seconds.
    #[clap(long)]
    test_receive: bool,
}

#[derive(Debug, Clone, Subcommand)]
enum Command {
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

    /// Print the contents of an .rrd file.
    Print(PrintCommand),

    /// Reset the memory of the Rerun Viewer.
    ///
    /// Only run this if you're having trouble with the Viewer,
    /// e.g. if it is crashing on startup.
    ///
    /// Rerun will forget all blueprints, as well as the native window's size, position and scale factor.
    #[cfg(feature = "native_viewer")]
    Reset,
}

#[derive(Debug, Clone, clap::Parser)]
struct PrintCommand {
    rrd_path: String,

    /// If specified, print out table contents.
    #[clap(long, short, default_value_t = false)]
    verbose: bool,
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
    let mut args = Args::parse_from(args);

    initialize_thread_pool(args.threads);

    if args.web_viewer {
        args.serve = true;
    }

    if args.version {
        println!("{build_info}");
        return Ok(0);
    }

    let res = if let Some(command) = &args.command {
        match command {
            #[cfg(feature = "analytics")]
            Command::Analytics(analytics) => run_analytics(analytics).map_err(Into::into),

            Command::Compare {
                path_to_rrd1,
                path_to_rrd2,
                full_dump,
            } => {
                let path_to_rrd1 = PathBuf::from(path_to_rrd1);
                let path_to_rrd2 = PathBuf::from(path_to_rrd2);
                run_compare(&path_to_rrd1, &path_to_rrd2, *full_dump)
            }

            Command::Print(print_command) => print_command.run(),

            #[cfg(feature = "native_viewer")]
            Command::Reset => re_viewer::reset_viewer_persistence(),
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

fn initialize_thread_pool(threads_args: i32) {
    // Name the rayon threads for the benefit of debuggers and profilers:
    let mut builder = rayon::ThreadPoolBuilder::new().thread_name(|i| format!("rayon-{i}"));

    if threads_args < 0 {
        match std::thread::available_parallelism() {
            Ok(cores) => {
                let threads = cores.get().saturating_sub((-threads_args) as _).max(1);
                re_log::debug!("Detected {cores} cores. Using {threads} compute threads.");
                builder = builder.num_threads(threads);
            }
            Err(err) => {
                re_log::warn!("Failed to query system of the number of cores: {err}.");
                // Let rayon decide for itself how many threads to use.
                // It's default is to use as many threads as we have cores,
                // (if rayon manages to figure out how many cores we have).
            }
        }
    } else {
        // 0 means "use all cores", and rayon understands that
        builder = builder.num_threads(threads_args as usize);
    }

    if let Err(err) = builder.build_global() {
        re_log::warn!("Failed to initialize rayon thread pool: {err}");
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
    fn compute_uber_table(
        path_to_rrd: &Path,
    ) -> anyhow::Result<(re_log_types::ApplicationId, re_log_types::DataTable)> {
        use re_entity_db::EntityDb;
        use re_log_types::StoreId;

        let rrd_file =
            std::fs::File::open(path_to_rrd).context("couldn't open rrd file contents")?;

        let mut stores: std::collections::HashMap<StoreId, EntityDb> = Default::default();
        let version_policy = re_log_encoding::decoder::VersionPolicy::Error;
        let decoder = re_log_encoding::decoder::Decoder::new(version_policy, rrd_file)?;
        for msg in decoder {
            let msg = msg.context("decode rrd message")?;
            stores
                .entry(msg.store_id().clone())
                .or_insert(re_entity_db::EntityDb::new(msg.store_id().clone()))
                .add(&msg)
                .context("decode rrd file contents")?;
        }

        let mut stores = stores
            .values()
            .filter(|store| store.store_kind() == re_log_types::StoreKind::Recording)
            .collect_vec();

        anyhow::ensure!(!stores.is_empty(), "no data recording found in rrd file");
        anyhow::ensure!(
            stores.len() == 1,
            "more than one data recording found in rrd file"
        );

        let store = stores.pop().unwrap(); // safe, ensured above

        Ok((
            store
                .app_id()
                .cloned()
                .unwrap_or_else(re_log_types::ApplicationId::unknown),
            store.store().to_data_table()?,
        ))
    }

    let (app_id1, table1) =
        compute_uber_table(path_to_rrd1).with_context(|| format!("path: {path_to_rrd1:?}"))?;
    let (app_id2, table2) =
        compute_uber_table(path_to_rrd2).with_context(|| format!("path: {path_to_rrd2:?}"))?;

    if full_dump {
        println!("{app_id1}");
        println!("{table1}");

        println!("{app_id2}");
        println!("{table2}");
    }

    anyhow::ensure!(
        app_id1 == app_id2,
        "Application IDs do not match: '{app_id1}' vs. '{app_id2}'"
    );

    re_log_types::DataTable::similar(&table1, &table2)
}

impl PrintCommand {
    fn run(&self) -> anyhow::Result<()> {
        let rrd_path = PathBuf::from(&self.rrd_path);
        self.print_rrd(&rrd_path)
            .with_context(|| format!("path: {rrd_path:?}"))
    }

    fn print_rrd(&self, rrd_path: &Path) -> anyhow::Result<()> {
        let Self {
            rrd_path: _,
            verbose,
        } = self;

        let rrd_file = std::fs::File::open(rrd_path)?;
        let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
        let decoder = re_log_encoding::decoder::Decoder::new(version_policy, rrd_file)?;
        for msg in decoder {
            let msg = msg.context("decode rrd message")?;
            match msg {
                LogMsg::SetStoreInfo(msg) => {
                    let SetStoreInfo { row_id: _, info } = msg;
                    println!("{info:#?}");
                }

                LogMsg::ArrowMsg(_row_id, arrow_msg) => {
                    let mut table =
                        DataTable::from_arrow_msg(&arrow_msg).context("Decode arrow message")?;

                    if *verbose {
                        println!("{table}");
                    } else {
                        table.compute_all_size_bytes();

                        let column_names =
                            table.columns.keys().map(|name| name.short_name()).join(" ");

                        let entity_paths = if table.col_entity_path.len() == 1 {
                            format!("{:?}", table.col_entity_path[0])
                        } else {
                            format!("{} different entity paths", table.col_entity_path.len())
                        };

                        println!(
                            "Table with {} rows ({}) - {entity_paths} - columns: [{column_names}]",
                            table.num_rows(),
                            re_format::format_bytes(table.heap_size_bytes() as _),
                        );
                    }
                }

                LogMsg::ActivateStore(store_id) => {
                    println!("ActivateStore({store_id})");
                }
            }
        }
        Ok(())
    }
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
    let startup_options = {
        re_tracing::profile_scope!("StartupOptions");
        re_viewer::StartupOptions {
            memory_limit: re_memory::MemoryLimit::parse(&args.memory_limit)
                .map_err(|err| anyhow::format_err!("Bad --memory-limit: {err}"))?,
            persist_state: args.persist_state,
            is_in_notebook: false,
            screenshot_to_path_then_quit: args.screenshot_to.clone(),

            skip_welcome_screen: args.skip_welcome_screen,

            // TODO(emilk): make it easy to set this on eframe instead
            resolution_in_points: if let Some(size) = &args.window_size {
                Some(parse_size(size)?)
            } else {
                None
            },
            force_wgpu_backend: None,
        }
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
            .map(|uri| DataSource::from_uri(re_log_types::FileSource::Cli, uri))
            .collect_vec();

        #[cfg(feature = "web_viewer")]
        if data_sources.len() == 1 && args.web_viewer {
            if let DataSource::WebSocketAddr(rerun_server_ws_url) = data_sources[0].clone() {
                // Special case! We are connecting a web-viewer to a web-socket address.
                // Instead of piping, just host a web-viewer that connects to the web-socket directly:
                return host_web_viewer(
                    args.bind.clone(),
                    args.web_viewer_port,
                    args.renderer,
                    true,
                    rerun_server_ws_url,
                )
                .await;
            }
        }

        data_sources
            .into_iter()
            .map(|data_source| data_source.stream(None))
            .collect::<Result<Vec<_>, _>>()?
    };

    // Now what do we do with the data?

    if args.test_receive {
        let rx = ReceiveSet::new(rx);
        assert_receive_into_entity_db(&rx).map(|_db| ())
    } else if let Some(rrd_path) = args.save {
        let rx = ReceiveSet::new(rx);
        Ok(stream_to_rrd_on_disk(&rx, &rrd_path.into())?)
    } else if args.serve {
        #[cfg(not(feature = "server"))]
        {
            _ = (call_source, rx);
            anyhow::bail!("Can't host server - rerun was not compiled with the 'server' feature");
        }

        #[cfg(not(feature = "web_viewer"))]
        if args.web_viewer {
            anyhow::bail!(
                "Can't host web-viewer - rerun was not compiled with the 'web_viewer' feature"
            );
        }

        #[cfg(feature = "server")]
        #[cfg(feature = "web_viewer")]
        if args.url_or_paths.is_empty()
            && (args.port == args.web_viewer_port.0 || args.port == args.ws_server_port.0)
        {
            anyhow::bail!(
                    "Trying to spawn a websocket server on {}, but this port is \
                    already used by the server we're connecting to. Please specify a different port.",
                    args.port
                );
        }

        #[cfg(feature = "server")]
        {
            let server_memory_limit = re_memory::MemoryLimit::parse(&args.server_memory_limit)
                .map_err(|err| anyhow::format_err!("Bad --server-memory-limit: {err}"))?;

            // This is the server which the web viewer will talk to:
            let ws_server = re_ws_comms::RerunServer::new(
                args.bind.clone(),
                args.ws_server_port,
                server_memory_limit,
            )
            .await?;
            let _ws_server_url = ws_server.server_url();
            let rx = ReceiveSet::new(rx);
            let ws_server_handle = tokio::spawn(ws_server.listen(rx));

            #[cfg(feature = "web_viewer")]
            {
                // We always host the web-viewer in case the users wants it,
                // but we only open a browser automatically with the `--web-viewer` flag.

                let open_browser = args.web_viewer;

                // This is the server that serves the Wasm+HTML:
                let web_server_handle = tokio::spawn(host_web_viewer(
                    args.bind.clone(),
                    args.web_viewer_port,
                    args.renderer,
                    open_browser,
                    _ws_server_url,
                ));

                // Wait for both servers to shutdown.
                web_server_handle.await?.map_err(anyhow::Error::from)?;
            }

            return ws_server_handle.await?.map_err(anyhow::Error::from);
        }
    } else {
        #[cfg(feature = "native_viewer")]
        return re_viewer::run_native_app(
            Box::new(move |cc, re_ui| {
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
                if let Ok(url) = std::env::var("EXAMPLES_MANIFEST_URL") {
                    app.set_examples_manifest_url(url);
                }
                Box::new(app)
            }),
            args.renderer,
        )
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
fn assert_receive_into_entity_db(
    rx: &ReceiveSet<LogMsg>,
) -> anyhow::Result<re_entity_db::EntityDb> {
    re_log::info!("Receiving messages into a EntityDb…");

    let mut rec: Option<re_entity_db::EntityDb> = None;
    let mut bp: Option<re_entity_db::EntityDb> = None;

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
                        let mut_db = match msg.store_id().kind {
                            re_log_types::StoreKind::Recording => rec.get_or_insert_with(|| {
                                re_entity_db::EntityDb::new(msg.store_id().clone())
                            }),
                            re_log_types::StoreKind::Blueprint => bp.get_or_insert_with(|| {
                                re_entity_db::EntityDb::new(msg.store_id().clone())
                            }),
                        };

                        mut_db.add(&msg)?;
                        num_messages += 1;
                    }
                    SmartMessagePayload::Quit(err) => {
                        if let Some(err) = err {
                            anyhow::bail!("data source has disconnected unexpectedly: {err}")
                        } else if let Some(db) = rec {
                            db.store().sanity_check()?;
                            anyhow::ensure!(0 < num_messages, "No messages received");
                            re_log::info!("Successfully ingested {num_messages} messages.");
                            return Ok(db);
                        } else {
                            anyhow::bail!("EntityDb never initialized");
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
