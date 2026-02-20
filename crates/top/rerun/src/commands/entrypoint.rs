use std::net::IpAddr;
use std::time::Duration;

use clap::{CommandFactory as _, Subcommand};
use itertools::Itertools as _;
use re_data_source::{AuthErrorHandler, LogDataSource};
use re_log_channel::{DataSourceMessage, LogReceiver, LogReceiverSet, SmartMessagePayload};
#[cfg(feature = "web_viewer")]
use re_sdk::web_viewer::WebViewerConfig;
use tokio::runtime::Runtime;

#[cfg(feature = "auth")]
use super::auth::AuthCommands;
use crate::CallSource;
#[cfg(feature = "analytics")]
use crate::commands::AnalyticsCommands;
#[cfg(feature = "data_loaders")]
use crate::commands::McapCommands;
use crate::commands::RrdCommands;

// ---

const LONG_ABOUT: &str = r#"
The Rerun command-line interface:
* Spawn viewers to visualize Rerun recordings and other supported formats.
* Start a gRPC server to share recordings over the network, on native or web.
* Inspect, edit and filter Rerun recordings.
"#;

// Place the important help _last_, to make it most visible in the terminal.
const ENVIRONMENT_VARIABLES_AND_EXAMPLES: &str = r#"
Environment variables:
    RERUN_CHUNK_MAX_BYTES     Maximum chunk size threshold for the compactor.
    RERUN_CHUNK_MAX_ROWS      Maximum chunk row count threshold for the compactor (sorted chunks).
    RERUN_CHUNK_MAX_ROWS_IF_UNSORTED
                              Maximum chunk row count threshold for the compactor (unsorted chunks).
    RERUN_SHADER_PATH         The search path for shader/shader-imports. Only available in developer builds.
    RERUN_TRACK_ALLOCATIONS   Track memory allocations to diagnose memory leaks in the viewer.
                              WARNING: slows down the viewer by a lot!
    RERUN_MAPBOX_ACCESS_TOKEN The Mapbox access token to use the Mapbox-provided backgrounds in the map view.
    RUST_LOG                  Change the log level of the viewer, e.g. `RUST_LOG=debug`.
    WGPU_BACKEND              Overwrites the graphics backend used, must be one of `vulkan`, `metal` or `gl`.
                              Default is `vulkan` everywhere except on Mac where we use `metal`. What is
                              supported depends on your OS.
    WGPU_POWER_PREF           Overwrites the power setting used for choosing a graphics adapter, must be `high`
                              or `low`. (Default is `high`)


Examples:
    Open a Rerun Viewer that listens for incoming SDK connections:
        rerun

    Load some files and show them in the Rerun Viewer:
        rerun recording.rrd mesh.obj image.png https://example.com/recording.rrd

    Open an .rrd file and stream it to a Web Viewer:
        rerun recording.rrd --web-viewer

    Host a Rerun gRPC server which listens for incoming connections from the logging SDK, buffer the log messages, and serve the results:
        rerun --serve-web

    Host a Rerun Server which serves a recording from a file over gRPC to any connecting Rerun Viewers:
        rerun --serve-web recording.rrd

    Host a Rerun gRPC server without spawning a Viewer:
        rerun --serve-grpc

    Spawn a Viewer without also hosting a gRPC server:
        rerun --connect

    Connect to a Rerun Server:
        rerun rerun+http://localhost:9877/proxy

    Listen for incoming gRPC connections from the logging SDK and stream the results to disk:
        rerun --save new_recording.rrd
"#;

#[derive(Debug, clap::Parser)]
#[clap(
    long_about = LONG_ABOUT,
    // Place most of the help last, as that is most visible in the terminal.
    after_long_help = ENVIRONMENT_VARIABLES_AND_EXAMPLES
)]
struct Args {
    // Note: arguments are sorted lexicographically for nicer `--help` message.
    //
    // We also use `long_help` on some arguments for more compact formatting.
    //
    #[command(subcommand)]
    command: Option<Command>,

    /// What bind address IP to use.
    ///
    /// `::` will listen on all interfaces, IPv6 and IPv4.
    #[clap(long, default_value = "0.0.0.0")]
    bind: IpAddr,

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
        default_value = "1GiB",
        long_help = r"An upper limit on how much memory the gRPC server (`--serve-web`) should use.
The server buffers log messages for the benefit of late-arriving viewers.
When this limit is reached, Rerun will drop the oldest data.
Example: `16GB` or `50%` (of system total)."
    )]
    server_memory_limit: String,

    /// If true, play back the most recent data first when new clients connect.
    #[clap(long)]
    newest_first: bool,

    #[clap(
        long,
        default_value_t = true,
        long_help = r"Whether the Rerun Viewer should persist the state of the viewer to disk.
When persisted, the state will be stored at the following locations:
- Linux: `/home/UserName/.local/share/rerun`
- macOS: `/Users/UserName/Library/Application Support/rerun`
- Windows: `C:\Users\UserName\AppData\Roaming\rerun`"
    )]
    persist_state: bool,

    /// What port do we listen to for SDKs to connect to over gRPC.
    // Default is `re_grpc_server::DEFAULT_SERVER_PORT`, can't use symbollically if `server` feature is disabled
    #[clap(long, default_value_t = 9876)]
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

    /// This will host a web-viewer over HTTP, and a gRPC server,
    /// unless one or more URIs are provided that can be viewed directly in the web viewer.
    ///
    /// If started, the web server will act like a proxy, listening for incoming connections from
    /// logging SDKs, and forwarding it to Rerun viewers.
    //
    // TODO(andreas): The Rust/Python APIs deprecated `serve_web` and instead encourage separate usage of `rec.serve_grpc()` + `rerun::serve_web_viewer()` instead.
    // It's worth considering doing the same here.
    #[clap(long)]
    serve_web: bool,

    /// This will host a gRPC server.
    ///
    /// The server will act like a proxy, listening for incoming connections from
    /// logging SDKs, and forwarding it to Rerun viewers.
    #[clap(long)]
    serve_grpc: bool,

    /// Do not attempt to start a new server, instead try to connect to an existing one.
    ///
    /// Optionally accepts a URL to a gRPC server.
    ///
    /// The scheme must be one of `rerun://`, `rerun+http://`, or `rerun+https://`,
    /// and the pathname must be `/proxy`.
    ///
    /// The default is `rerun+http://127.0.0.1:9876/proxy`.
    #[clap(long)]
    #[expect(clippy::option_option)] // Tri-state: none, --connect, --connect <url>.
    connect: Option<Option<String>>,

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
- A gRPC url to a Rerun server
- A path to a Rerun .rrd recording
- A path to a Rerun .rbl blueprint
- An HTTP(S) URL to an .rrd or .rbl file to load
- A path to an image or mesh, or any other file that Rerun can load (see https://www.rerun.io/docs/reference/data-loaders/overview)

If no arguments are given, a server will be hosted which a Rerun SDK can connect to.")]
    url_or_paths: Vec<String>,

    /// Print version and quit.
    #[clap(long)]
    version: bool,

    /// Start the viewer in the browser (instead of locally).
    ///
    /// Requires Rerun to have been compiled with the `web_viewer` feature.
    ///
    /// This implies `--serve-web`.
    #[clap(long)]
    web_viewer: bool,

    /// What port do we listen to for hosting the web viewer over HTTP.
    /// A port of 0 will pick a random port.
    // Default is `re_web_viewer_server::DEFAULT_WEB_VIEWER_SERVER_PORT`, can't use symbollically if `web_viewer` feature is disabled
    #[clap(long, default_value_t = 9090)]
    web_viewer_port: u16,

    /// Hide the normal Rerun welcome screen.
    #[clap(long)]
    hide_welcome_screen: bool,

    /// Detach Rerun Viewer process from the application process.
    #[clap(long)]
    detach_process: bool,

    /// Set the screen resolution (in logical points), e.g. "1920x1080".
    /// Useful together with `--screenshot-to`.
    #[clap(long)]
    window_size: Option<String>,

    /// Override the default graphics backend and for a specific one instead.
    ///
    /// When using `--web-viewer` this should be one of: `webgpu`, `webgl`.
    ///
    /// When starting a native viewer instead this should be one of:
    ///
    /// * `vulkan` (Linux & Windows only)
    ///
    /// * `gl` (Linux & Windows only)
    ///
    /// * `metal` (macOS only)
    //
    // Note that we don't compile with DX12 right now, but we could (we don't since this adds permutation and wgpu still has some issues with it).
    // GL could be enabled on MacOS via `angle` but given prior issues with ANGLE this seems to be a bad idea!
    #[clap(long)]
    renderer: Option<String>,

    /// Overwrites hardware acceleration option for video decoding.
    ///
    /// By default uses the last provided setting, which is `auto` if never configured.
    ///
    /// Depending on the decoder backend, these settings are merely hints and may be ignored.
    /// However, they can be useful in some situations to work around issues.
    ///
    /// Possible values:
    ///
    /// * `auto`
    ///   May use hardware acceleration if available and compatible with the codec.
    ///
    /// * `prefer_software`
    ///   Should use a software decoder even if hardware acceleration is available.
    ///   If no software decoder is present, this may cause decoding to fail.
    ///
    /// * `prefer_hardware`
    ///   Should use a hardware decoder.
    ///   If no hardware decoder is present, this may cause decoding to fail.
    #[clap(long, verbatim_doc_comment)]
    video_decoder: Option<String>,

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

impl Args {
    fn generate_markdown_manual() -> String {
        let mut out = String::new();

        fn generate_arg_doc(arg: &clap::Arg) -> String {
            let mut names = Vec::new();
            if let Some(short) = arg.get_short() {
                names.push(format!("-{short}"));
            }
            if let Some(long) = arg.get_long() {
                names.push(format!("--{long}"));
            }

            let values = arg.get_value_names().map_or_else(String::new, |values| {
                values
                    .iter()
                    .map(|v| format!("<{v}>"))
                    .collect_vec()
                    .join(", ")
            });

            let help = if let Some(help) = arg.get_long_help() {
                Some(
                    help.to_string()
                        .lines()
                        .map(|line| format!("> {line}").trim().to_owned())
                        .collect_vec()
                        .join("\n"),
                )
            } else {
                arg.get_help().map(|help| {
                    if help.to_string().ends_with('?') {
                        format!("> {help}")
                    } else {
                        format!("> {help}.")
                    }
                    .trim()
                    .to_owned()
                })
            };

            let rendered = if names.is_empty() {
                format!("* `{values}`")
            } else {
                format!("* `{} {values}`", names.join(", "))
            }
            .trim()
            .to_owned();

            let rendered = if let Some(help) = help {
                format!("{rendered}\n{help}")
            } else {
                rendered
            }
            .trim()
            .to_owned();

            let defaults = arg.get_default_values();
            if defaults.is_empty() {
                rendered
            } else {
                let defaults = defaults
                    .iter()
                    .map(|v| format!("`{}`", v.to_string_lossy().trim()))
                    .collect_vec()
                    .join(", ");
                format!("{rendered}\n>\n> [Default: {defaults}]")
                    .trim()
                    .to_owned()
            }
        }

        fn generate_markdown_manual(
            full_name: Vec<String>,
            out: &mut String,
            cmd: &mut clap::Command,
        ) {
            let name = cmd.get_name();

            if name == "help" {
                return;
            }

            let any_subcommands = cmd.get_subcommands().any(|cmd| cmd.get_name() != "help");
            let any_positional_args = cmd.get_arguments().any(|arg| arg.is_positional());
            let any_floating_args = cmd.get_arguments().any(|arg| {
                !arg.is_positional() && !arg.is_hide_set() && arg.get_long() != Some("help")
            });

            let full_name = full_name
                .into_iter()
                .chain(std::iter::once(name.to_owned()))
                .collect_vec();

            if !any_positional_args && !any_floating_args && !any_subcommands {
                return;
            }

            // E.g. "## rerun analytics"
            let header = format!("{} {}", "##", full_name.join(" "))
                .trim()
                .to_owned();

            // E.g. "**Usage**: `rerun [OPTIONS] [URL_OR_PATHS]… [COMMAND]`"
            let usage = {
                let usage = cmd.render_usage().to_string();
                let (_, usage) = usage.split_at(7);
                let full_name = {
                    let mut full_name = full_name.clone();
                    _ = full_name.pop();
                    full_name
                };

                let mut rendered = String::new();
                if let Some(about) = cmd.get_long_about() {
                    rendered += &format!("{about}\n\n");
                } else if let Some(about) = cmd.get_about() {
                    rendered += &format!("{about}.\n\n");
                }
                rendered += format!("**Usage**: `{} {usage}`", full_name.join(" ")).trim();

                rendered
            };

            // E.g.:
            // """
            // **Commands**
            //
            // * `analytics`: Configure the behavior of our analytics
            // * `rrd`: Manipulate the contents of .rrd and .rbl files
            // * `reset`: Reset the memory of the Rerun Viewer
            // """
            let commands = any_subcommands.then(|| {
                let commands = cmd
                    .get_subcommands_mut()
                    .filter(|cmd| cmd.get_name() != "help")
                    .map(|cmd| {
                        let name = cmd.get_name().to_owned();
                        let help = cmd.render_help().to_string();
                        let help = help.split_once('\n').map_or("", |(help, _)| help).trim();
                        // E.g. "`analytics`:  Configure the behavior of our analytics"
                        format!("* `{name}`: {help}.")
                    })
                    .collect_vec()
                    .join("\n");

                format!("**Commands**\n\n{commands}")
            });

            // E.g.:
            // """
            // **Arguments**
            //
            // `[URL_OR_PATHS]…`
            // > Any combination of:
            // > - A gRPC url to a Rerun server
            // > - A path to a Rerun .rrd recording
            // > - A path to a Rerun .rbl blueprint
            // > - An HTTP(S) URL to an .rrd or .rbl file to load
            // > - A path to an image or mesh, or any other file that Rerun can load (see https://www.rerun.io/docs/reference/data-loaders/overview)
            // >
            // > If no arguments are given, a server will be hosted which a Rerun SDK can connect to.
            // """
            let positionals = any_positional_args.then(|| {
                let arguments = cmd
                    .get_arguments()
                    .filter(|arg| arg.is_positional())
                    .map(generate_arg_doc)
                    .collect_vec()
                    .join("\n\n");

                format!("**Arguments**\n\n{arguments}")
            });

            // E.g.:
            // """
            // **Options**
            //
            // `--bind <BIND>`
            // > What bind address IP to use.
            // >
            // > [default: ::]
            // """
            let floatings = any_floating_args.then(|| {
                let options = cmd
                    .get_arguments()
                    .filter(|arg| {
                        !arg.is_positional() && !arg.is_hide_set() && arg.get_long() != Some("help")
                    })
                    .map(generate_arg_doc)
                    .collect_vec()
                    .join("\n\n");

                format!("**Options**\n\n{options}")
            });

            *out += &[Some(header), Some(usage), commands, positionals, floatings]
                .into_iter()
                .flatten()
                .collect_vec()
                .join("\n\n");

            *out += "\n\n";

            for cmd in cmd.get_subcommands_mut() {
                generate_markdown_manual(full_name.clone(), out, cmd);
            }
        }

        generate_markdown_manual(Vec::new(), &mut out, &mut Self::command());

        out.trim().replace("...", "…") // NOLINT
    }
}

// Commands sorted alphabetically:
#[derive(Debug, Clone, Subcommand)]
enum Command {
    /// Configure the behavior of our analytics.
    #[cfg(feature = "analytics")]
    #[command(subcommand)]
    Analytics(AnalyticsCommands),

    /// Authentication with the redap.
    #[cfg(feature = "auth")]
    #[command(subcommand)]
    Auth(AuthCommands),

    /// Generates the Rerun CLI manual (markdown).
    ///
    /// Example: `rerun man > docs/content/reference/cli.md`
    #[command(name = "man")]
    Manual,

    #[cfg(feature = "data_loaders")]
    #[command(subcommand)]
    Mcap(McapCommands),

    /// Reset the memory of the Rerun Viewer.
    ///
    /// Only run this if you're having trouble with the Viewer,
    /// e.g. if it is crashing on startup.
    ///
    /// Rerun will forget all blueprints, as well as the native window's size, position and scale factor.
    #[cfg(feature = "native_viewer")]
    Reset,

    #[command(subcommand)]
    Rrd(RrdCommands),

    /// In-memory Rerun data server
    #[cfg(feature = "oss_server")]
    #[command(name = "server")]
    Server(re_server::Args),
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
    main_thread_token: crate::MainThreadToken,
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

    #[cfg(not(target_arch = "wasm32"))]
    if cfg!(feature = "perf_telemetry") && re_log::env_var_is_truthy("TELEMETRY_ENABLED") {
        eprintln!("Disabling crash handler because of perf_telemetry/TELEMETRY_ENABLED"); // Ask Clement why
    } else {
        re_crash_handler::install_crash_handlers(build_info.clone());
    }

    // There is always value in setting this, even if `re_perf_telemetry` is disabled. For example,
    // the Rerun versioning headers will automatically pick it up.
    //
    // Safety: anything touching the env is unsafe, tis what it is.
    #[expect(unsafe_code)]
    unsafe {
        std::env::set_var("OTEL_SERVICE_NAME", "rerun");
    }

    use clap::Parser as _;
    let mut args = Args::parse_from(args);

    #[cfg(feature = "analytics")]
    record_cli_command_analytics(&args);

    initialize_thread_pool(args.threads);

    if args.web_viewer {
        args.serve_web = true;
    }

    if args.version {
        println!("{build_info}");
        println!(
            "Video features: {}",
            re_video::enabled_features().iter().join(" ")
        );
        return Ok(0);
    }

    #[cfg(feature = "native_viewer")]
    let profiler = run_profiler(&args);

    // We don't want the runtime to run on the main thread, as we need that one for our UI.
    // So we can't call `block_on` anywhere in the entrypoint - we must call `tokio::spawn`
    // and synchronize the result using some other means instead.
    let tokio_runtime = initialize_tokio_runtime(args.threads)?;
    let _tokio_guard = tokio_runtime.enter();

    let res = if let Some(command) = args.command {
        match command {
            #[cfg(feature = "auth")]
            Command::Auth(cmd) => cmd.run(tokio_runtime.handle()).map_err(Into::into),

            #[cfg(feature = "analytics")]
            Command::Analytics(analytics) => analytics.run().map_err(Into::into),

            Command::Manual => {
                let man = Args::generate_markdown_manual();
                let web_header = unindent::unindent(
                    "\
                    ---
                    title: ⌨️ CLI manual
                    order: 1150
                    ---\
                    ",
                );
                println!("{web_header}\n\n{man}");
                Ok(())
            }

            #[cfg(feature = "data_loaders")]
            Command::Mcap(mcap) => mcap.run(),

            #[cfg(feature = "native_viewer")]
            Command::Reset => re_viewer::reset_viewer_persistence(),

            Command::Rrd(rrd) => rrd.run(),

            #[cfg(feature = "oss_server")]
            Command::Server(server) => tokio_runtime.block_on(server.run_async()),
        }
    } else {
        #[cfg(all(not(target_arch = "wasm32"), feature = "perf_telemetry"))]
        let mut _telemetry = {
            // NOTE: We're just parsing the environment, hence the `vec![]` for CLI flags.
            use re_perf_telemetry::external::clap::Parser as _;
            let args = re_perf_telemetry::TelemetryArgs::parse_from::<_, String>(vec![]);

            // Remember: telemetry must be init in a Tokio context.
            tokio_runtime.block_on(async {
                re_perf_telemetry::Telemetry::init(
                    args,
                    re_perf_telemetry::TelemetryDropBehavior::Shutdown,
                )
                // Perf telemetry is a developer tool, it's not compiled into final user builds.
                .expect("could not start perf telemetry")
            })

            // TODO(tokio-rs/tracing#3239): The viewer will crash on exit because of what appears
            // to be a design flaw in `tracing-subscriber`'s shutdown implementation, specifically
            // it assumes that all the relevant thread-local state will be dropped in the proper
            // order, when really it won't and there's no way to guarantee that.
            // See <https://github.com/tokio-rs/tracing/issues/3239>.
            //
            // What happens in practice will depend on what you and all your dependencies are
            // doing. This problem has been seen before specifically for egui apps [1], but really
            // it has nothing to do with egui per se.
            // [1]: <https://github.com/smol-rs/polling/issues/231>
            //
            // Since this is a very niche feature only meant to be used for deep performance work,
            // I think this is fine for now (and I don't think there's anything we can do from
            // userspace anyhow, this is a pure `tracing` issue, unrelated to `re_perf_telemetry`).
        };

        run_impl(
            main_thread_token,
            build_info,
            call_source,
            args,
            tokio_runtime.handle(),
            #[cfg(feature = "native_viewer")]
            profiler,
        )
    };

    match res {
        // Clean success
        Ok(_) => Ok(0),

        // Clean failure -- known error AddrInUse
        Err(err)
            if err
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io_err| io_err.kind() == std::io::ErrorKind::AddrInUse) =>
        {
            re_log::warn!("{err}");
            Ok(1)
        }

        // Unclean failure -- re-raise exception
        Err(err) => Err(err),
    }
}

fn run_impl(
    _main_thread_token: crate::MainThreadToken,
    _build_info: re_build_info::BuildInfo,
    _call_source: CallSource,
    args: Args,
    tokio_runtime_handle: &tokio::runtime::Handle,
    #[cfg(feature = "native_viewer")] profiler: re_tracing::Profiler,
) -> anyhow::Result<()> {
    //TODO(#10068): populate token passed with `--token`
    let connection_registry = re_redap_client::ConnectionRegistry::new_with_stored_credentials();

    let server_addr = std::net::SocketAddr::new(args.bind, args.port);

    #[cfg(feature = "server")]
    let server_options = re_sdk::ServerOptions {
        playback_behavior: re_sdk::PlaybackBehavior::from_newest_first(args.newest_first),

        memory_limit: {
            re_log::debug!("Parsing --server-memory-limit (for gRPC server)");
            let limit = args.server_memory_limit.as_str();
            re_log::debug!("Server memory limit: {limit}");
            re_memory::MemoryLimit::parse(limit)
                .map_err(|err| anyhow::format_err!("Bad --server-memory-limit: {err}"))?
        },
    };

    // All URLs that we want to process.
    #[allow(clippy::allow_attributes, unused_mut)]
    let mut url_or_paths = args.url_or_paths.clone();

    // Passing `--connect` accounts to adding a proxy URL to the list of URLs that we want to process.
    #[cfg(feature = "server")]
    if let Some(url) = args.connect.clone() {
        let url = url.unwrap_or_else(|| format!("rerun+http://{server_addr}/proxy"));
        if let Err(err) = url.as_str().parse::<re_uri::RedapUri>() {
            anyhow::bail!("expected `/proxy` endpoint: {err}");
        }
        url_or_paths.push(url);
    }

    // Now what do we do with the data?
    if args.test_receive || args.save.is_some() {
        save_or_test_receive(
            args.save,
            url_or_paths,
            &connection_registry,
            #[cfg(feature = "server")]
            server_addr,
            #[cfg(feature = "server")]
            server_options,
        )
    } else if args.serve_grpc {
        cfg_if::cfg_if! {
            if #[cfg(feature = "server")] {
                serve_grpc(
                    url_or_paths,
                    tokio_runtime_handle,
                    &connection_registry,
                    server_addr,
                    server_options,
                )
            } else {
                Err(anyhow::anyhow!(
                    "rerun-cli must be compiled with the 'server' feature enabled"
                ))
            }
        }
    } else if args.serve_web {
        cfg_if::cfg_if! {
            if #[cfg(not(feature = "server"))] {
                Err(anyhow::anyhow!(
                    "Can't host server - rerun was not compiled with the 'server' feature"
                ))
            } else if #[cfg(not(feature = "web_viewer"))] {
                Err(anyhow::anyhow!(
                    "Can't host web-viewer - rerun was not compiled with the 'web_viewer' feature"
                ))
            } else {
                // We always host the web-viewer in case the users wants it,
                // but we only open a browser automatically with the `--web-viewer` flag.
                let open_browser = args.web_viewer;

                #[cfg(all(feature = "server", feature = "web_viewer"))]
                serve_web(
                    url_or_paths,
                    &connection_registry,
                    args.web_viewer_port,
                    args.renderer,
                    args.video_decoder,
                    server_addr,
                    server_options,
                    open_browser,
                )
            }
        }
    } else if args.connect.is_none() && is_another_server_already_running(server_addr) {
        connect_to_existing_server(url_or_paths, &connection_registry, server_addr)
    } else {
        cfg_if::cfg_if! {
            if #[cfg(feature = "native_viewer")] {
                start_native_viewer(
                    &args,
                    url_or_paths,
                    _main_thread_token,
                    _build_info,
                    _call_source,
                    tokio_runtime_handle,
                    profiler,
                    connection_registry,
                    #[cfg(feature = "server")]
                    server_addr,
                    #[cfg(feature = "server")]
                    server_options,
                )
            } else {
                Err(anyhow::anyhow!(
                    "Can't start viewer - rerun was compiled without the 'native_viewer' feature"
                ))
            }
        }
    }
}

#[cfg(feature = "native_viewer")]
#[expect(clippy::too_many_arguments)]
#[allow(clippy::allow_attributes, unused_variables)]
fn start_native_viewer(
    args: &Args,
    url_or_paths: Vec<String>,
    _main_thread_token: re_viewer::MainThreadToken,
    _build_info: re_build_info::BuildInfo,
    call_source: CallSource,
    tokio_runtime_handle: &tokio::runtime::Handle,
    profiler: re_tracing::Profiler,
    connection_registry: re_redap_client::ConnectionRegistryHandle,
    #[cfg(feature = "server")] server_addr: std::net::SocketAddr,
    #[cfg(feature = "server")] server_options: re_sdk::ServerOptions,
) -> anyhow::Result<()> {
    use re_viewer::external::re_viewer_context;

    use crate::external::re_ui::{UICommand, UICommandSender as _};

    let startup_options = native_startup_options_from_args(args)?;

    let connect = args.connect.is_some();
    let renderer = args.renderer.as_deref();

    let (command_tx, command_rx) = re_viewer_context::command_channel();

    let auth_error_handler = re_viewer::App::auth_error_handler(command_tx.clone());

    let tokio_runtime_handle = tokio_runtime_handle.clone();

    // Start catching `re_log::info/warn/error` messages
    // so we can show them in the notification panel.
    // In particular: create this before calling `run_native_app`
    // so we catch any warnings produced during startup.
    let text_log_rx = re_viewer::register_text_log_receiver();

    re_viewer::run_native_app(
        _main_thread_token,
        Box::new(move |cc| {
            {
                let tx = command_tx.clone();
                let egui_ctx = cc.egui_ctx.clone();
                tokio::spawn(async move {
                    // We catch ctrl-c commands so we can properly quit.
                    // Without this, recent state changes might not be persisted.
                    match tokio::signal::ctrl_c().await {
                        Ok(()) => {
                            re_log::info!("Caught Ctrl-C, quitting Rerun Viewer…");
                            tx.send_ui(UICommand::Quit);
                            egui_ctx.request_repaint();
                        }
                        Err(err) => {
                            re_log::error!("Failed to listen for ctrl-c signal: {err}");
                        }
                    }
                });
            }
            let mut app = re_viewer::App::with_commands(
                _main_thread_token,
                _build_info,
                call_source.app_env(),
                startup_options,
                cc,
                Some(connection_registry.clone()),
                re_viewer::AsyncRuntimeHandle::new_native(tokio_runtime_handle),
                text_log_rx,
                (command_tx, command_rx),
            );

            #[allow(clippy::allow_attributes, unused_mut)]
            let ReceiversFromUrlParams {
                mut log_receivers,
                urls_to_pass_on_to_viewer,
            } = ReceiversFromUrlParams::new(
                url_or_paths,
                &UrlParamProcessingConfig::native_viewer(),
                &connection_registry,
                Some(auth_error_handler),
            )?;

            // If we're **not** connecting to an existing server, we spawn a new one and add it to the list of receivers.
            #[cfg(feature = "server")]
            if !connect {
                let log_receiver = re_grpc_server::spawn_with_recv(
                    server_addr,
                    server_options,
                    re_grpc_server::shutdown::never(),
                );

                log_receivers.push(log_receiver);
            }

            app.set_profiler(profiler);
            for rx in log_receivers {
                app.add_log_receiver(rx);
            }
            for url in urls_to_pass_on_to_viewer {
                app.open_url_or_file(&url);
            }
            if let Ok(url) = std::env::var("EXAMPLES_MANIFEST_URL") {
                app.set_examples_manifest_url(url);
            }

            Ok(Box::new(app))
        }),
        renderer,
    )
    .map_err(|err| err.into())
}

#[cfg(feature = "native_viewer")]
fn native_startup_options_from_args(args: &Args) -> anyhow::Result<re_viewer::StartupOptions> {
    re_tracing::profile_function!();

    let video_decoder_hw_acceleration = args.video_decoder.as_ref().and_then(|s| match s.parse() {
        Err(()) => {
            re_log::warn_once!("Failed to parse --video-decoder value: {s}. Ignoring.");
            None
        }
        Ok(hw_accell) => Some(hw_accell),
    });

    Ok(re_viewer::StartupOptions {
        hide_welcome_screen: args.hide_welcome_screen,
        detach_process: args.detach_process,
        memory_limit: {
            re_log::debug!("Parsing --memory-limit (for Viewer)");
            re_memory::MemoryLimit::parse(&args.memory_limit)
                .map_err(|err| anyhow::format_err!("Bad --memory-limit: {err}"))?
        },
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
        force_wgpu_backend: args.renderer.clone(),
        video_decoder_hw_acceleration,

        ..Default::default()
    })
}

fn connect_to_existing_server(
    url_or_paths: Vec<String>,
    connection_registry: &re_redap_client::ConnectionRegistryHandle,
    server_addr: std::net::SocketAddr,
) -> anyhow::Result<()> {
    use re_sdk::sink::LogSink as _;

    let uri: re_uri::ProxyUri = format!("rerun+http://{server_addr}/proxy").parse()?;
    re_log::info!(%uri, "Another viewer is already running, streaming data to it.");
    let sink = re_sdk::sink::GrpcSink::new(uri);
    let receivers = ReceiversFromUrlParams::new(
        url_or_paths,
        &UrlParamProcessingConfig::convert_everything_to_data_sources(),
        connection_registry,
        None,
    )?;
    if !receivers.urls_to_pass_on_to_viewer.is_empty() {
        re_log::warn!(
            "The following URLs can't be passed to already open viewers yet: {:?}",
            receivers.urls_to_pass_on_to_viewer
        );
    }
    for rx in receivers.log_receivers {
        while rx.is_connected() {
            while let Ok(msg) = rx.recv() {
                if let Some(msg) = msg.into_data() {
                    match msg {
                        DataSourceMessage::LogMsg(log_msg) => {
                            sink.send(log_msg);
                        }
                        unsupported => {
                            re_log::error_once!(
                                "Can't pass on {} to the server",
                                unsupported.variant_name()
                            );
                        }
                    }
                }
            }
        }
    }
    sink.flush_blocking(Duration::MAX)?;

    Ok(())
}

#[expect(clippy::too_many_arguments)]
#[cfg(all(feature = "server", feature = "web_viewer"))]
fn serve_web(
    url_or_paths: Vec<String>,
    connection_registry: &re_redap_client::ConnectionRegistryHandle,
    web_viewer_port: u16,
    force_wgpu_backend: Option<String>,
    video_decoder: Option<String>,
    server_addr: std::net::SocketAddr,
    server_options: re_sdk::ServerOptions,
    open_browser: bool,
) -> anyhow::Result<()> {
    let ReceiversFromUrlParams {
        log_receivers,
        mut urls_to_pass_on_to_viewer,
    } = ReceiversFromUrlParams::new(
        url_or_paths,
        &UrlParamProcessingConfig::grpc_server_and_web_viewer(),
        connection_registry,
        None,
    )?;

    // Don't spawn a server if there's only a bunch of URIs that we want to view directly.
    let spawn_server = !log_receivers.is_empty() || urls_to_pass_on_to_viewer.is_empty();
    if spawn_server {
        if server_addr.port() == web_viewer_port {
            anyhow::bail!(
                "Trying to spawn a Web Viewer server on {}, but this port is \
                    already used by the server we're connecting to. Please specify a different port.",
                server_addr.port()
            );
        }

        // Spawn a server which the Web Viewer can connect to.
        // All `rxs` are consumed by the server.
        re_grpc_server::spawn_from_rx_set(
            server_addr,
            server_options,
            re_grpc_server::shutdown::never(),
            LogReceiverSet::new(log_receivers),
        );

        // Add the proxy URL to the url parameters.
        let proxy_url = if server_addr.ip().is_unspecified() || server_addr.ip().is_loopback() {
            format!("rerun+http://localhost:{}/proxy", server_addr.port())
        } else {
            format!("rerun+http://{server_addr}/proxy")
        };

        re_log::debug_assert!(
            proxy_url.parse::<re_uri::RedapUri>().is_ok(),
            "Expected a proper proxy URI, but got {proxy_url:?}"
        );

        urls_to_pass_on_to_viewer.push(proxy_url);
    }

    // This is the server that serves the Wasm+HTML:
    WebViewerConfig {
        bind_ip: server_addr.ip().to_string(),
        web_port: re_web_viewer_server::WebViewerServerPort(web_viewer_port),
        connect_to: urls_to_pass_on_to_viewer,
        force_wgpu_backend,
        video_decoder,
        open_browser,
    }
    .host_web_viewer()?
    .block();

    Ok(())
}

#[cfg(feature = "server")]
fn serve_grpc(
    url_or_paths: Vec<String>,
    tokio_runtime_handle: &tokio::runtime::Handle,
    connection_registry: &re_redap_client::ConnectionRegistryHandle,
    server_addr: std::net::SocketAddr,
    server_options: re_sdk::ServerOptions,
) -> anyhow::Result<()> {
    if !cfg!(feature = "server") {
        anyhow::bail!("Can't host server - rerun was not compiled with the 'server' feature");
    }

    let receivers = ReceiversFromUrlParams::new(
        url_or_paths,
        &UrlParamProcessingConfig::convert_everything_to_data_sources(),
        connection_registry,
        None,
    )?;
    receivers.error_on_unhandled_urls("--serve-grpc")?;

    let (signal, shutdown) = re_grpc_server::shutdown::shutdown();
    // Spawn a server which the Web Viewer can connect to.
    re_grpc_server::spawn_from_rx_set(
        server_addr,
        server_options,
        shutdown,
        LogReceiverSet::new(receivers.log_receivers),
    );

    // Gracefully shut down the server on SIGINT
    tokio_runtime_handle.block_on(tokio::signal::ctrl_c()).ok();

    signal.stop();

    Ok(())
}

fn save_or_test_receive(
    save: Option<String>,
    url_or_paths: Vec<String>,
    connection_registry: &re_redap_client::ConnectionRegistryHandle,
    #[cfg(feature = "server")] server_addr: std::net::SocketAddr,
    #[cfg(feature = "server")] server_options: re_sdk::ServerOptions,
) -> anyhow::Result<()> {
    let receivers = ReceiversFromUrlParams::new(
        url_or_paths,
        &UrlParamProcessingConfig::convert_everything_to_data_sources(),
        connection_registry,
        None,
    )?;
    receivers.error_on_unhandled_urls(if save.is_none() {
        "--test-receive"
    } else {
        "--save"
    })?;

    #[allow(clippy::allow_attributes, unused_mut)]
    let mut log_receivers = receivers.log_receivers;

    #[cfg(feature = "server")]
    {
        let log_rx = re_grpc_server::spawn_with_recv(
            server_addr,
            server_options,
            re_grpc_server::shutdown::never(),
        );

        log_receivers.push(log_rx);
    }

    let receive_set = LogReceiverSet::new(log_receivers);

    if let Some(rrd_path) = save {
        Ok(stream_to_rrd_on_disk(&receive_set, &rrd_path.into())?)
    } else {
        assert_receive_into_entity_db(&receive_set).map(|_db| ())
    }
}

fn is_another_server_already_running(server_addr: std::net::SocketAddr) -> bool {
    // Check if there is already a viewer running and if so, send the data to it.
    use std::net::TcpStream;
    if TcpStream::connect_timeout(&server_addr, std::time::Duration::from_secs(1)).is_ok() {
        re_log::info!(
            %server_addr,
            "A process is already listening at this address. Assuming it's a Rerun Viewer."
        );
        true
    } else {
        false
    }
}

// NOTE: This is only used as part of end-to-end tests.
fn assert_receive_into_entity_db(rx: &LogReceiverSet) -> anyhow::Result<re_entity_db::EntityDb> {
    re_log::info!("Receiving messages into a EntityDb…");

    let mut rec: Option<re_entity_db::EntityDb> = None;
    let mut bp: Option<re_entity_db::EntityDb> = None;

    let mut num_messages = 0;

    let timeout = std::time::Duration::from_secs(12);

    loop {
        if !rx.is_connected() {
            anyhow::bail!("Channel disconnected without a Goodbye message.");
        }

        if let Some((_, msg)) = rx.recv_timeout(timeout) {
            re_log::info_once!("Received first message.");

            match msg.payload {
                SmartMessagePayload::Msg(msg) => {
                    match msg {
                        DataSourceMessage::RrdManifest(store_id, rrd_manifest) => {
                            let mut_db = match store_id.kind() {
                                re_log_types::StoreKind::Recording => {
                                    rec.get_or_insert_with(|| {
                                        re_entity_db::EntityDb::new(store_id.clone())
                                    })
                                }
                                re_log_types::StoreKind::Blueprint => bp.get_or_insert_with(|| {
                                    re_entity_db::EntityDb::new(store_id.clone())
                                }),
                            };

                            mut_db.add_rrd_manifest_message(rrd_manifest);
                        }

                        DataSourceMessage::LogMsg(msg) => {
                            let mut_db = match msg.store_id().kind() {
                                re_log_types::StoreKind::Recording => {
                                    rec.get_or_insert_with(|| {
                                        re_entity_db::EntityDb::new(msg.store_id().clone())
                                    })
                                }
                                re_log_types::StoreKind::Blueprint => bp.get_or_insert_with(|| {
                                    re_entity_db::EntityDb::new(msg.store_id().clone())
                                }),
                            };

                            mut_db.add_log_msg(&msg)?;
                        }

                        DataSourceMessage::TableMsg(_) => {
                            anyhow::bail!(
                                "Received a TableMsg which can't be stored in an EntityDb"
                            );
                        }

                        DataSourceMessage::UiCommand(ui_command) => {
                            anyhow::bail!(
                                "Received a UI command which can't be stored in an EntityDb: {ui_command:?}"
                            );
                        }
                    }

                    num_messages += 1;
                }

                re_log_channel::SmartMessagePayload::Flush { on_flush_done } => {
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
        } else {
            if let Some(db) = rec {
                // TODO(RR-3373): find a proper way to detect client disconnect without timing out.
                re_log::info!(
                    "Timed out after successfully receiving {num_messages} messages. Assuming the client disconnected cleanly.",
                );
                return Ok(db);
            }
            anyhow::bail!(
                "Didn't receive any messages within {} seconds. Giving up.",
                timeout.as_secs()
            );
        }
    }
}

// --- util ---

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
    } else if threads_args == 1 {
        // 1 means "single-threaded".
        // Put all jobs on the caller thread, just to simplify
        // the flamegraph and make it more similar to a browser.
        builder = builder.num_threads(1).use_current_thread();
        re_log::info!("Running in single-threaded mode.");
    } else {
        // 0 means "use all cores", and rayon understands that
        builder = builder.num_threads(threads_args as usize);
    }

    if let Err(err) = builder.build_global() {
        re_log::warn!("Failed to initialize rayon thread pool: {err}");
    }
}

fn initialize_tokio_runtime(threads_args: i32) -> std::io::Result<Runtime> {
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Name the tokio threads for the benefit of debuggers and profilers:
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.thread_name_fn(|| {
        static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
        let nr = ATOMIC_ID.fetch_add(1, Ordering::Relaxed);
        format!("tokio-#{nr}")
    });
    builder.enable_all();

    if threads_args < 0 {
        if let Ok(cores) = std::thread::available_parallelism() {
            let threads = cores.get().saturating_sub((-threads_args) as _).max(1);
            builder.worker_threads(threads);
        }
    } else if 0 < threads_args {
        builder.worker_threads(threads_args as usize);
    } else {
        // 0 means "use default" (typically num CPUs)
    }

    builder.build()
}

#[cfg(feature = "native_viewer")]
fn run_profiler(args: &Args) -> re_tracing::Profiler {
    let mut profiler = re_tracing::Profiler::default();
    if args.profile {
        profiler.start();
    }
    profiler
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
        .ok_or_else(|| anyhow::anyhow!("Invalid size {size:?}, expected e.g. 800x600"))
}

// --- io ---

// TODO(cmc): dedicated module for io utils, especially stdio streaming in and out.

fn stream_to_rrd_on_disk(
    rx: &re_log_channel::LogReceiverSet,
    path: &std::path::PathBuf,
) -> Result<(), re_log_encoding::FileSinkError> {
    use re_log_encoding::FileSinkError;

    if path.exists() {
        re_log::warn!(?path, "Overwriting existing file");
    }

    re_log::info!("Saving incoming log stream to {path:?}. Abort with Ctrl-C.");

    let encoding_options = re_log_encoding::rrd::EncodingOptions::PROTOBUF_COMPRESSED;
    let file = std::fs::File::create(path).map_err(|err| FileSinkError::CreateFile {
        path: path.clone(),
        source: err,
    })?;
    let mut encoder = re_log_encoding::Encoder::new_eager(
        re_build_info::CrateVersion::LOCAL,
        encoding_options,
        file,
    )?;

    loop {
        if let Ok(msg) = rx.recv() {
            if let Some(payload) = msg.into_data() {
                match payload {
                    DataSourceMessage::LogMsg(log_msg) => {
                        encoder.append(&log_msg)?;
                    }
                    unsupported => {
                        re_log::error_once!(
                            "Received a {} which can't be stored in a file",
                            unsupported.variant_name()
                        );
                    }
                }
            }
        } else {
            re_log::info!("Log stream disconnected, stopping.");
            break;
        }
    }

    re_log::info!("File saved to {path:?}");

    Ok(())
}

/// Describes how to handle URLs passed on the CLI.
struct UrlParamProcessingConfig {
    data_sources_from_http_urls: bool,
    data_sources_from_redap_datasets: bool,
    data_source_from_filepaths: bool,
}

impl UrlParamProcessingConfig {
    /// Instruct to create data sources for everything we can.
    ///
    /// This is used for pure servers and file redirects.
    fn convert_everything_to_data_sources() -> Self {
        // Write to file makes everything it can a data source.
        Self {
            data_sources_from_http_urls: true,
            data_sources_from_redap_datasets: true,
            data_source_from_filepaths: true,
        }
    }

    #[allow(clippy::allow_attributes, dead_code)] // May be unused depending on feature flags.
    fn grpc_server_and_web_viewer() -> Self {
        // GRPC with web viewer can handle everything except files directly.
        Self {
            data_sources_from_http_urls: false,
            data_sources_from_redap_datasets: false,
            data_source_from_filepaths: true,
        }
    }

    #[allow(clippy::allow_attributes, dead_code)] // May be unused depending on feature flags.
    fn native_viewer() -> Self {
        // Native viewer passes everything on to the viewer unchanged.
        Self {
            data_sources_from_http_urls: false,
            data_sources_from_redap_datasets: false,
            data_source_from_filepaths: false,
        }
    }
}

/// Log receivers created from URLs or path parameters that were passed in on the CLI.
struct ReceiversFromUrlParams {
    /// Log receivers that we want to hook up to a connection or viewer.
    log_receivers: Vec<LogReceiver>,

    /// URLs that should be passed on to the viewer if possible.
    ///
    /// If we can't do that, we should error or warn, see [`Self::error_on_unhandled_urls`].
    urls_to_pass_on_to_viewer: Vec<String>,
}

impl ReceiversFromUrlParams {
    /// Processes all incoming URLs according to the given config.
    fn new(
        input_urls: Vec<String>,
        config: &UrlParamProcessingConfig,
        connection_registry: &re_redap_client::ConnectionRegistryHandle,
        auth_error_handler: Option<AuthErrorHandler>,
    ) -> anyhow::Result<Self> {
        let mut data_sources = Vec::new();
        let mut urls_to_pass_on_to_viewer = Vec::new();

        for url in input_urls {
            if let Some(data_source) = LogDataSource::from_uri(re_log_types::FileSource::Cli, &url)
            {
                match &data_source {
                    LogDataSource::HttpUrl { .. } => {
                        if config.data_sources_from_http_urls {
                            data_sources.push(data_source);
                        } else {
                            urls_to_pass_on_to_viewer.push(url);
                        }
                    }

                    LogDataSource::RedapProxy(..) | LogDataSource::RedapDatasetSegment { .. } => {
                        if config.data_sources_from_redap_datasets {
                            data_sources.push(data_source);
                        } else {
                            urls_to_pass_on_to_viewer.push(url);
                        }
                    }

                    LogDataSource::FilePath(..) => {
                        if config.data_source_from_filepaths {
                            data_sources.push(data_source);
                        } else {
                            urls_to_pass_on_to_viewer.push(url);
                        }
                    }

                    LogDataSource::FileContents(..) | LogDataSource::Stdin => {
                        data_sources.push(data_source);
                    }
                }
            } else {
                // We don't have the full url parsing logic here. Just pass it on to the viewer!
                urls_to_pass_on_to_viewer.push(url);
            }
        }

        let auth_error_handler = auth_error_handler.unwrap_or_else(|| {
            std::sync::Arc::new(|uri, err| {
                re_log::error!(?uri, "Authentication error for data source: {err}");
            })
        });

        let log_receivers = data_sources
            .into_iter()
            .map(|data_source| data_source.stream(auth_error_handler.clone(), connection_registry))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(Self {
            log_receivers,
            urls_to_pass_on_to_viewer,
        })
    }

    /// Returns an error if there are any URLs that weren't converted into log receivers.
    fn error_on_unhandled_urls(&self, command: &str) -> anyhow::Result<()> {
        if !self.urls_to_pass_on_to_viewer.is_empty() {
            anyhow::bail!(
                "`{command}` does not support these URLs: {:?}",
                self.urls_to_pass_on_to_viewer
            );
        }
        Ok(())
    }
}

/// Records analytics for the CLI command invocation.
#[cfg(feature = "analytics")]
fn record_cli_command_analytics(args: &Args) {
    let Some(analytics) = re_analytics::Analytics::global_or_init() else {
        return;
    };

    // Destructure to ensure we consider all fields when adding new ones.
    let Args {
        command,
        newest_first,
        persist_state,
        profile,
        save,
        screenshot_to,
        serve_web,
        serve_grpc,
        connect,
        expect_data_soon,
        test_receive,
        hide_welcome_screen,
        detach_process,

        // Not logged
        threads: _,
        url_or_paths: _,
        version: _,
        web_viewer,
        web_viewer_port: _,
        window_size: _,
        renderer: _,
        video_decoder: _,
        bind: _,
        memory_limit: _,
        server_memory_limit: _,
        port: _,
    } = args;

    let (command, subcommand) = match command {
        #[cfg(feature = "analytics")]
        Some(Command::Analytics(cmd)) => {
            let subcommand = match cmd {
                AnalyticsCommands::Details => "details",
                AnalyticsCommands::Clear => "clear",
                AnalyticsCommands::Email { .. } => "email",
                AnalyticsCommands::Enable => "enable",
                AnalyticsCommands::Disable => "disable",
                AnalyticsCommands::Config => "config",
            };
            ("analytics", Some(subcommand))
        }

        #[cfg(feature = "auth")]
        Some(Command::Auth(cmd)) => {
            let subcommand = match cmd {
                AuthCommands::Login(_) => "login",
                AuthCommands::Logout(_) => "logout",
                AuthCommands::Token(_) => "token",
                AuthCommands::GenerateToken(_) => "generate-token",
            };
            ("auth", Some(subcommand))
        }

        Some(Command::Manual) => ("man", None),

        #[cfg(feature = "data_loaders")]
        Some(Command::Mcap(cmd)) => {
            let subcommand = match cmd {
                McapCommands::Convert(_) => "convert",
            };
            ("mcap", Some(subcommand))
        }

        #[cfg(feature = "native_viewer")]
        Some(Command::Reset) => ("reset", None),

        Some(Command::Rrd(cmd)) => {
            let subcommand = match cmd {
                RrdCommands::Compact(_) => "compact",
                RrdCommands::Compare(_) => "compare",
                RrdCommands::Filter(_) => "filter",
                RrdCommands::Merge(_) => "merge",
                RrdCommands::Migrate(_) => "migrate",
                RrdCommands::Print(_) => "print",
                RrdCommands::Route(_) => "route",
                RrdCommands::Stats(_) => "stats",
                RrdCommands::Verify(_) => "verify",
            };
            ("rrd", Some(subcommand))
        }

        #[cfg(feature = "oss_server")]
        Some(Command::Server(_)) => ("server", None),

        None => ("viewer", None),
    };

    analytics.record(re_analytics::event::CliCommandInvoked {
        command,
        subcommand,
        web_viewer: *web_viewer,
        serve_web: *serve_web,
        serve_grpc: *serve_grpc,
        connect: connect.is_some(),
        save: save.is_some(),
        screenshot_to: screenshot_to.is_some(),
        newest_first: *newest_first,
        persist_state_disabled: !persist_state,
        profile: *profile,
        expect_data_soon: *expect_data_soon,
        hide_welcome_screen: *hide_welcome_screen,
        detach_process: *detach_process,
        test_receive: *test_receive,
    });
}
