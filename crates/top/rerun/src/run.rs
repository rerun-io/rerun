use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context as _;
use clap::Subcommand;
use itertools::{izip, Itertools};

use re_data_source::DataSource;
use re_log_types::{LogMsg, SetStoreInfo};
use re_sdk::log::Chunk;
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
    /// We use this to generate screenshots of our examples.
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

    /// This is a hint that we expect a recording to stream in very soon.
    ///
    /// This is set by the `spawn()` method in our logging SDK.
    ///
    /// The viewer will respond by fading in the welcome screen,
    /// instead of showing it directly.
    /// This ensures that it won't blink for a few frames before switching to the recording.
    #[clap(long)]
    expect_data_soon: bool,

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
- A WebSocket url to a Rerun server
- A path to a Rerun .rrd recording
- A path to a Rerun .rbl blueprint
- An HTTP(S) URL to an .rrd or .rbl file to load
- A path to an image or mesh, or any other file that Rerun can load (see https://www.rerun.io/docs/reference/data-loaders/overview)

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

    /// Hide the normal Rerun welcome screen.
    #[clap(long)]
    hide_welcome_screen: bool,

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

    #[command(subcommand)]
    Rrd(RrdCommands),

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

#[derive(Debug, Clone, Subcommand)]
enum RrdCommands {
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

    /// Print the contents of an .rrd or .rbl file.
    Print(PrintCommand),

    /// Compacts the contents of an .rrd or .rbl file and writes the result to a new file.
    ///
    /// Use the usual environment variables to control the compaction thresholds:
    /// `RERUN_CHUNK_MAX_ROWS`,
    /// `RERUN_CHUNK_MAX_ROWS_IF_UNSORTED`,
    /// `RERUN_CHUNK_MAX_BYTES`.
    ///
    /// Example: `RERUN_CHUNK_MAX_ROWS=4096 RERUN_CHUNK_MAX_BYTES=1048576 rerun compact -i input.rrd -o output.rrd`
    Compact {
        #[arg(short = 'i', long = "input", value_name = "src.(rrd|rbl)")]
        path_to_input_rrd: String,

        #[arg(short = 'o', long = "output", value_name = "dst.(rrd|rbl)")]
        path_to_output_rrd: String,
    },

    /// Merges the contents of multiple .rrd and/or .rbl files, and writes the result to a new file.
    ///
    /// Example: `rerun merge -i input1.rrd -i input2.rbl -i input3.rrd -o output.rrd`
    Merge {
        #[arg(
            short = 'i',
            long = "input",
            value_name = "src.(rrd|rbl)",
            required = true
        )]
        path_to_input_rrds: Vec<String>,

        #[arg(short = 'o', long = "output", value_name = "dst.(rrd|rbl)")]
        path_to_output_rrd: String,
    },
}

/// Where are we calling [`run`] from?
// TODO(jleibs): Maybe remove call-source all together.
// However, this context of spawn vs direct CLI-invocation still seems
// useful for analytics. We just need to capture the data some other way.
pub enum CallSource {
    /// Called from a command-line-input (the terminal).
    Cli,
}

impl CallSource {
    #[cfg(feature = "native_viewer")]
    fn app_env(&self) -> re_viewer::AppEnvironment {
        match self {
            Self::Cli => re_viewer::AppEnvironment::RerunCli {
                rustc_version: env!("RE_BUILD_RUSTC_VERSION").into(),
                llvm_version: env!("RE_BUILD_LLVM_VERSION").into(),
            },
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
pub fn run<I, T>(
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
            Command::Analytics(analytics) => run_analytics_commands(analytics).map_err(Into::into),

            Command::Rrd(rrd) => run_rrd_commands(rrd),

            #[cfg(feature = "native_viewer")]
            Command::Reset => re_viewer::reset_viewer_persistence(),
        }
    } else {
        run_impl(build_info, call_source, args)
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
                // Its default is to use as many threads as we have cores,
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

fn run_rrd_commands(cmd: &RrdCommands) -> anyhow::Result<()> {
    match cmd {
        RrdCommands::Compare {
            path_to_rrd1,
            path_to_rrd2,
            full_dump,
        } => {
            let path_to_rrd1 = PathBuf::from(path_to_rrd1);
            let path_to_rrd2 = PathBuf::from(path_to_rrd2);
            run_compare(&path_to_rrd1, &path_to_rrd2, *full_dump)
                // Print current directory, this can be useful for debugging issues with relative paths.
                .with_context(|| format!("current directory {:?}", std::env::current_dir()))
        }

        RrdCommands::Print(print_command) => print_command.run(),

        RrdCommands::Compact {
            path_to_input_rrd,
            path_to_output_rrd,
        } => {
            let path_to_input_rrd = PathBuf::from(path_to_input_rrd);
            let path_to_output_rrd = PathBuf::from(path_to_output_rrd);
            run_compact(&path_to_input_rrd, &path_to_output_rrd)
        }

        RrdCommands::Merge {
            path_to_input_rrds,
            path_to_output_rrd,
        } => {
            let path_to_input_rrds = path_to_input_rrds.iter().map(PathBuf::from).collect_vec();
            let path_to_output_rrd = PathBuf::from(path_to_output_rrd);
            run_merge(&path_to_input_rrds, &path_to_output_rrd)
        }
    }
}

/// Checks whether two .rrd files are _similar_, i.e. not equal on a byte-level but
/// functionally equivalent.
///
/// Returns `Ok(())` if they match, or an error containing a detailed diff otherwise.
fn run_compare(path_to_rrd1: &Path, path_to_rrd2: &Path, full_dump: bool) -> anyhow::Result<()> {
    /// Given a path to an rrd file, builds up a `ChunkStore` and returns its contents a stream of
    /// `Chunk`s.
    ///
    /// Fails if there are more than one data recordings present in the rrd file.
    fn compute_uber_table(
        path_to_rrd: &Path,
    ) -> anyhow::Result<(re_log_types::ApplicationId, Vec<Arc<re_chunk::Chunk>>)> {
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
                .or_insert_with(|| re_entity_db::EntityDb::new(msg.store_id().clone()))
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
            store.store().iter_chunks().map(Arc::clone).collect_vec(),
        ))
    }

    let (app_id1, chunks1) = compute_uber_table(path_to_rrd1).with_context(|| {
        format!(
            "path: {path_to_rrd1:?} (absolute: {:?})",
            std::fs::canonicalize(path_to_rrd1) // Print absolute path as well, since we encountered issues with this on CI.
        )
    })?;
    let (app_id2, chunks2) = compute_uber_table(path_to_rrd2).with_context(|| {
        format!(
            "path: {path_to_rrd2:?} (absolute: {:?})",
            std::fs::canonicalize(path_to_rrd2) // Print absolute path as well, since we encountered issues with this on CI.
        )
    })?;

    if full_dump {
        println!("{app_id1}");
        for chunk in &chunks1 {
            println!("{chunk}");
        }

        println!("{app_id2}");
        for chunk in &chunks2 {
            println!("{chunk}");
        }
    }

    anyhow::ensure!(
        app_id1 == app_id2,
        "Application IDs do not match: '{app_id1}' vs. '{app_id2}'"
    );

    anyhow::ensure!(
        chunks1.len() == chunks2.len(),
        "Number of Chunks does not match: '{}' vs. '{}'",
        re_format::format_uint(chunks1.len()),
        re_format::format_uint(chunks2.len()),
    );

    for (chunk1, chunk2) in izip!(chunks1, chunks2) {
        anyhow::ensure!(
            re_chunk::Chunk::are_similar(&chunk1, &chunk2),
            "Chunks do not match:\n{}",
            similar_asserts::SimpleDiff::from_str(
                &format!("{chunk1}"),
                &format!("{chunk2}"),
                "got",
                "expected",
            ),
        );
    }

    Ok(())
}

fn run_compact(path_to_input_rrd: &Path, path_to_output_rrd: &Path) -> anyhow::Result<()> {
    use re_entity_db::EntityDb;
    use re_log_types::StoreId;

    let rrd_in =
        std::fs::File::open(path_to_input_rrd).with_context(|| format!("{path_to_input_rrd:?}"))?;
    let rrd_in_size = rrd_in.metadata().ok().map(|md| md.len());

    let file_size_to_string = |size: Option<u64>| {
        size.map_or_else(
            || "<unknown>".to_owned(),
            |size| re_format::format_bytes(size as _),
        )
    };

    use re_chunk_store::ChunkStoreConfig;
    let mut store_config = ChunkStoreConfig::from_env().unwrap_or_default();
    // NOTE: We're doing headless processing, there's no point in running subscribers, it will just
    // (massively) slow us down.
    store_config.enable_changelog = false;

    re_log::info!(
        src = ?path_to_input_rrd,
        src_size_bytes = %file_size_to_string(rrd_in_size),
        dst = ?path_to_output_rrd,
        max_num_rows = %re_format::format_uint(store_config.chunk_max_rows),
        max_num_bytes = %re_format::format_bytes(store_config.chunk_max_bytes as _),
        "compaction started"
    );

    let now = std::time::Instant::now();

    let mut entity_dbs: std::collections::HashMap<StoreId, EntityDb> = Default::default();
    let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
    let decoder = re_log_encoding::decoder::Decoder::new(version_policy, rrd_in)?;
    let version = decoder.version();
    for msg in decoder {
        let msg = msg.context("decode rrd message")?;
        entity_dbs
            .entry(msg.store_id().clone())
            .or_insert_with(|| {
                re_entity_db::EntityDb::with_store_config(
                    msg.store_id().clone(),
                    store_config.clone(),
                )
            })
            .add(&msg)
            .context("decode rrd file contents")?;
    }

    anyhow::ensure!(
        !entity_dbs.is_empty(),
        "no recordings found in rrd/rbl file"
    );

    let mut rrd_out = std::fs::File::create(path_to_output_rrd)
        .with_context(|| format!("{path_to_output_rrd:?}"))?;

    let messages: Result<Vec<Vec<LogMsg>>, _> = entity_dbs
        .into_values()
        .map(|entity_db| entity_db.to_messages(None /* time selection */))
        .collect();
    let messages = messages?;
    let messages = messages.iter().flatten();

    let encoding_options = re_log_encoding::EncodingOptions::COMPRESSED;
    re_log_encoding::encoder::encode(version, encoding_options, messages, &mut rrd_out)
        .context("Message encode")?;

    let rrd_out_size = rrd_out.metadata().ok().map(|md| md.len());

    let compaction_ratio =
        if let (Some(rrd_in_size), Some(rrd_out_size)) = (rrd_in_size, rrd_out_size) {
            format!(
                "{:3.3}%",
                100.0 - rrd_out_size as f64 / (rrd_in_size as f64 + f64::EPSILON) * 100.0
            )
        } else {
            "N/A".to_owned()
        };

    re_log::info!(
        src = ?path_to_input_rrd,
        src_size_bytes = %file_size_to_string(rrd_in_size),
        dst = ?path_to_output_rrd,
        dst_size_bytes = %file_size_to_string(rrd_out_size),
        time = ?now.elapsed(),
        compaction_ratio,
        "compaction finished"
    );

    Ok(())
}

fn run_merge(path_to_input_rrds: &[PathBuf], path_to_output_rrd: &Path) -> anyhow::Result<()> {
    use re_entity_db::EntityDb;
    use re_log_types::StoreId;

    let rrds_in: Result<Vec<_>, _> = path_to_input_rrds
        .iter()
        .map(|path_to_input_rrd| {
            std::fs::File::open(path_to_input_rrd).with_context(|| format!("{path_to_input_rrd:?}"))
        })
        .collect();
    let rrds_in = rrds_in?;

    let rrds_in_size = rrds_in
        .iter()
        .map(|rrd_in| rrd_in.metadata().ok().map(|md| md.len()))
        .sum::<Option<u64>>();

    let file_size_to_string = |size: Option<u64>| {
        size.map_or_else(
            || "<unknown>".to_owned(),
            |size| re_format::format_bytes(size as _),
        )
    };

    use re_chunk_store::ChunkStoreConfig;
    let mut store_config = ChunkStoreConfig::from_env().unwrap_or_default();
    // NOTE: We're doing headless processing, there's no point in running subscribers, it will just
    // (massively) slow us down.
    store_config.enable_changelog = false;

    re_log::info!(
        srcs = ?path_to_input_rrds,
        dst = ?path_to_output_rrd,
        max_num_rows = %re_format::format_uint(store_config.chunk_max_rows),
        max_num_bytes = %re_format::format_bytes(store_config.chunk_max_bytes as _),
        "merge started"
    );

    let now = std::time::Instant::now();

    let mut entity_dbs: std::collections::HashMap<StoreId, EntityDb> = Default::default();
    let mut version = None;
    for rrd_in in rrds_in {
        let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
        let decoder = re_log_encoding::decoder::Decoder::new(version_policy, rrd_in)?;
        version = version.max(Some(decoder.version()));
        for msg in decoder {
            let msg = msg.context("decode rrd message")?;
            entity_dbs
                .entry(msg.store_id().clone())
                .or_insert_with(|| {
                    re_entity_db::EntityDb::with_store_config(
                        msg.store_id().clone(),
                        store_config.clone(),
                    )
                })
                .add(&msg)
                .context("decode rrd file contents")?;
        }
    }

    anyhow::ensure!(
        !entity_dbs.is_empty(),
        "no recordings found in rrd/rbl files"
    );

    let mut rrd_out = std::fs::File::create(path_to_output_rrd)
        .with_context(|| format!("{path_to_output_rrd:?}"))?;

    let messages: Result<Vec<Vec<LogMsg>>, _> = entity_dbs
        .into_values()
        .map(|entity_db| entity_db.to_messages(None /* time selection */))
        .collect();
    let messages = messages?;
    let messages = messages.iter().flatten();

    let encoding_options = re_log_encoding::EncodingOptions::COMPRESSED;
    let version = version.unwrap_or(re_build_info::CrateVersion::LOCAL);
    re_log_encoding::encoder::encode(version, encoding_options, messages, &mut rrd_out)
        .context("Message encode")?;

    let rrd_out_size = rrd_out.metadata().ok().map(|md| md.len());

    re_log::info!(
        srcs = ?path_to_input_rrds,
        srcs_size_bytes = %file_size_to_string(rrds_in_size),
        dst = ?path_to_output_rrd,
        dst_size_bytes = %file_size_to_string(rrd_out_size),
        time = ?now.elapsed(),
        "merge finished"
    );

    Ok(())
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
        println!("Decoded RRD stream v{}\n---", decoder.version());
        for msg in decoder {
            let msg = msg.context("decode rrd message")?;
            match msg {
                LogMsg::SetStoreInfo(msg) => {
                    let SetStoreInfo { row_id: _, info } = msg;
                    println!("{info:#?}");
                }

                LogMsg::ArrowMsg(_row_id, arrow_msg) => {
                    let chunk = match Chunk::from_arrow_msg(&arrow_msg) {
                        Ok(chunk) => chunk,
                        Err(err) => {
                            eprintln!("discarding broken chunk: {err}");
                            continue;
                        }
                    };

                    if *verbose {
                        println!("{chunk}");
                    } else {
                        let column_names = chunk
                            .component_names()
                            .map(|name| name.short_name())
                            .join(" ");

                        println!(
                            "Chunk with {} rows ({}) - {:?} - columns: [{column_names}]",
                            chunk.num_rows(),
                            re_format::format_bytes(chunk.total_size_bytes() as _),
                            chunk.entity_path(),
                        );
                    }
                }

                LogMsg::BlueprintActivationCommand(re_log_types::BlueprintActivationCommand {
                    blueprint_id,
                    make_active,
                    make_default,
                }) => {
                    println!("BlueprintActivationCommand({blueprint_id}, make_active: {make_active}, make_default: {make_default})");
                }
            }
        }
        Ok(())
    }
}

#[cfg(feature = "analytics")]
fn run_analytics_commands(cmd: &AnalyticsCommands) -> Result<(), re_analytics::cli::CliError> {
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

fn run_impl(
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
            hide_welcome_screen: args.hide_welcome_screen,
            memory_limit: re_memory::MemoryLimit::parse(&args.memory_limit)
                .map_err(|err| anyhow::format_err!("Bad --memory-limit: {err}"))?,
            persist_state: args.persist_state,
            is_in_notebook: false,
            screenshot_to_path_then_quit: args.screenshot_to.clone(),

            expect_data_soon: if args.expect_data_soon {
                Some(true)
            } else {
                None
            },

            // TODO(emilk): make it easy to set this on eframe instead
            resolution_in_points: if let Some(size) = &args.window_size {
                Some(parse_size(size)?)
            } else {
                None
            },
            force_wgpu_backend: None,

            panel_state_overrides: Default::default(),
        }
    };

    // Where do we get the data from?
    let rx: Vec<Receiver<LogMsg>> = if args.url_or_paths.is_empty() {
        #[cfg(feature = "server")]
        {
            let server_options = re_sdk_comms::ServerOptions {
                max_latency_sec: parse_max_latency(args.drop_at_latency.as_ref()),
                quiet: false,
            };
            let rx = re_sdk_comms::serve(&args.bind, args.port, server_options)?;
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
                host_web_viewer(
                    &args.bind,
                    args.web_viewer_port,
                    args.renderer,
                    true,
                    &rerun_server_ws_url,
                )?
                .block();

                return Ok(());
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
            let _ws_server = re_ws_comms::RerunServer::new(
                ReceiveSet::new(rx),
                &args.bind,
                args.ws_server_port,
                server_memory_limit,
            )?;

            #[cfg(feature = "web_viewer")]
            {
                // We always host the web-viewer in case the users wants it,
                // but we only open a browser automatically with the `--web-viewer` flag.

                let open_browser = args.web_viewer;

                // This is the server that serves the Wasm+HTML:
                host_web_viewer(
                    &args.bind,
                    args.web_viewer_port,
                    args.renderer,
                    open_browser,
                    &_ws_server.server_url(),
                )?
                .block(); // dropping should stop the server
            }

            return Ok(());
        }
    } else {
        #[cfg(feature = "native_viewer")]
        return re_viewer::run_native_app(
            Box::new(move |cc| {
                let mut app = re_viewer::App::new(
                    _build_info,
                    &call_source.app_env(),
                    startup_options,
                    cc.egui_ctx.clone(),
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

                    re_smart_channel::SmartMessagePayload::Flush { on_flush_done } => {
                        on_flush_done();
                    }

                    SmartMessagePayload::Quit(err) => {
                        if let Some(err) = err {
                            anyhow::bail!("data source has disconnected unexpectedly: {err}")
                        } else if let Some(db) = rec {
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
    let mut encoder = re_log_encoding::encoder::Encoder::new(
        re_build_info::CrateVersion::LOCAL,
        encoding_options,
        file,
    )?;

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
