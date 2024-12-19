use clap::{CommandFactory, Subcommand};
use itertools::Itertools;

use re_data_source::DataSource;
use re_log_types::LogMsg;
use re_sdk::sink::LogSink;
use re_smart_channel::{ReceiveSet, Receiver, SmartMessagePayload};

use crate::{commands::RrdCommands, CallSource};

#[cfg(feature = "web_viewer")]
use re_sdk::web_viewer::WebViewerConfig;
#[cfg(feature = "web_viewer")]
use re_web_viewer_server::WebViewerServerPort;
#[cfg(feature = "server")]
use re_ws_comms::RerunServerPort;

#[cfg(feature = "analytics")]
use crate::commands::AnalyticsCommands;

// ---

const LONG_ABOUT: &str = r#"
The Rerun command-line interface:
* Spawn viewers to visualize Rerun recordings and other supported formats.
* Start TCP and WebSocket servers to share recordings over the network, on native or web.
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

    Host a Rerun TCP server which listens for incoming TCP connections from the logging SDK, buffer the log messages, and serves the results over WebSockets:
        rerun --serve-web

    Host a Rerun Server which serves a recording over WebSocket to any connecting Rerun Viewers:
        rerun --serve-web recording.rrd

    Connect to a Rerun Server:
        rerun ws://localhost:9877

    Listen for incoming TCP connections from the logging SDK and stream the results to disk:
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
        long_help = r"An upper limit on how much memory the WebSocket server (`--serve-web`) should use.
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
- Linux: `/home/UserName/.local/share/rerun`
- macOS: `/Users/UserName/Library/Application Support/rerun`
- Windows: `C:\Users\UserName\AppData\Roaming\rerun`"
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

    /// Deprecated: use `--serve-web` instead.
    #[clap(long)]
    serve: bool,

    /// Serve the recordings over WebSocket to one or more Rerun Viewers.
    ///
    /// This will also host a web-viewer over HTTP that can connect to the WebSocket address,
    /// but you can also connect with the native binary.
    ///
    /// `rerun --serve-web` will act like a proxy, listening for incoming TCP connection from
    /// logging SDKs, and forwarding it to Rerun viewers.
    #[clap(long)]
    serve_web: bool,

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
    /// Requires Rerun to have been compiled with the `web_viewer` feature.
    ///
    /// This implies `--serve-web`.
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
            let any_floating_args = cmd
                .get_arguments()
                .any(|arg| !arg.is_positional() && arg.get_long() != Some("help"));

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

            // E.g. "**Usage**: `rerun [OPTIONS] [URL_OR_PATHS]... [COMMAND]`"
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
            // > - A WebSocket url to a Rerun server
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
            // > What bind address IP to use
            // >
            // > [default: 0.0.0.0]
            //
            // `--drop-at-latency <DROP_AT_LATENCY>`
            // > Set a maximum input latency, e.g. "200ms" or "10s".
            // >
            // > If we go over this, we start dropping packets.
            // >
            // > The default is no limit, which means Rerun might eat more and more memory and have longer and longer latency, if you are logging data faster than Rerun can index it.
            // """
            let floatings = any_floating_args.then(|| {
                let options = cmd
                    .get_arguments()
                    .filter(|arg| !arg.is_positional() && arg.get_long() != Some("help"))
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

        out.trim().replace("...", "…")
    }
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

    /// Generates the Rerun CLI manual (markdown).
    ///
    /// Example: `rerun man > docs/content/reference/cli.md`
    #[command(name = "man")]
    Manual,
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

    re_crash_handler::install_crash_handlers(build_info);

    use clap::Parser as _;
    let mut args = Args::parse_from(args);

    initialize_thread_pool(args.threads);

    if args.web_viewer {
        args.serve = true;
        args.serve_web = true;
    }

    if args.version {
        println!("{build_info}");
        println!("Video features: {}", re_video::build_info().features);
        return Ok(0);
    }

    let res = if let Some(command) = &args.command {
        match command {
            #[cfg(feature = "analytics")]
            Command::Analytics(analytics) => analytics.run().map_err(Into::into),

            Command::Rrd(rrd) => rrd.run(),

            #[cfg(feature = "native_viewer")]
            Command::Reset => re_viewer::reset_viewer_persistence(),

            Command::Manual => {
                let man = Args::generate_markdown_manual();
                let web_header = unindent::unindent(
                    "\
                    ---
                    title: CLI manual
                    order: 250
                    ---\
                    ",
                );
                println!("{web_header}\n\n{man}");
                Ok(())
            }
        }
    } else {
        run_impl(main_thread_token, build_info, call_source, args)
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

fn run_impl(
    _main_thread_token: crate::MainThreadToken,
    _build_info: re_build_info::BuildInfo,
    call_source: CallSource,
    args: Args,
) -> anyhow::Result<()> {
    #[cfg(feature = "native_viewer")]
    let profiler = run_profiler(&args);
    let mut is_another_viewer_running = false;

    #[cfg(feature = "native_viewer")]
    let startup_options = {
        re_tracing::profile_scope!("StartupOptions");

        let video_decoder_hw_acceleration =
            args.video_decoder.as_ref().and_then(|s| match s.parse() {
                Err(()) => {
                    re_log::warn_once!("Failed to parse --video-decoder value: {s}. Ignoring.");
                    None
                }
                Ok(hw_accell) => Some(hw_accell),
            });

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
            force_wgpu_backend: args.renderer.clone(),
            video_decoder_hw_acceleration,

            panel_state_overrides: Default::default(),
        }
    };

    // Where do we get the data from?
    let rxs: Vec<Receiver<LogMsg>> = {
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

                WebViewerConfig {
                    bind_ip: args.bind,
                    web_port: args.web_viewer_port,
                    source_url: Some(rerun_server_ws_url),
                    force_wgpu_backend: args.renderer,
                    video_decoder: args.video_decoder,
                    open_browser: true,
                }
                .host_web_viewer()?
                .block();

                return Ok(());
            }
        }

        let mut rxs = data_sources
            .into_iter()
            .map(|data_source| data_source.stream(None))
            .collect::<Result<Vec<_>, _>>()?;

        #[cfg(feature = "server")]
        {
            // Check if there is already a viewer running and if so, send the data to it.
            use std::net::TcpStream;
            let addr = std::net::SocketAddr::new(re_sdk::default_server_addr().ip(), args.port);
            if TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(1)).is_ok() {
                re_log::info!(
                    %addr,
                    "A process is already listening at this address. Assuming it's a Rerun Viewer."
                );
                is_another_viewer_running = true;
            } else {
                let server_options = re_sdk_comms::ServerOptions {
                    max_latency_sec: parse_max_latency(args.drop_at_latency.as_ref()),
                    quiet: false,
                };
                let tcp_listener: Receiver<LogMsg> =
                    re_sdk_comms::serve(&args.bind, args.port, server_options)?;
                rxs.push(tcp_listener);
            }
        }

        rxs
    };

    // Now what do we do with the data?

    if args.test_receive {
        let rx = ReceiveSet::new(rxs);
        assert_receive_into_entity_db(&rx).map(|_db| ())
    } else if let Some(rrd_path) = args.save {
        let rx = ReceiveSet::new(rxs);
        Ok(stream_to_rrd_on_disk(&rx, &rrd_path.into())?)
    } else if args.serve || args.serve_web {
        #[cfg(not(feature = "server"))]
        {
            _ = (call_source, rxs);
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
                ReceiveSet::new(rxs),
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
                WebViewerConfig {
                    bind_ip: args.bind,
                    web_port: args.web_viewer_port,
                    source_url: Some(_ws_server.server_url()),
                    force_wgpu_backend: args.renderer,
                    video_decoder: args.video_decoder,
                    open_browser,
                }
                .host_web_viewer()?
                .block(); // dropping should stop the server
            }

            #[cfg(not(feature = "web_viewer"))]
            {
                // Returning from this function so soon would drop and therefore stop the server.
                _ws_server.block();
            }

            return Ok(());
        }
    } else if is_another_viewer_running {
        let addr = std::net::SocketAddr::new(re_sdk::default_server_addr().ip(), args.port);
        re_log::info!(%addr, "Another viewer is already running, streaming data to it.");

        let sink = re_sdk::sink::TcpSink::new(addr, re_sdk::default_flush_timeout());

        for rx in rxs {
            while rx.is_connected() {
                while let Ok(msg) = rx.recv() {
                    if let Some(log_msg) = msg.into_data() {
                        sink.send(log_msg);
                    }
                }
            }
        }

        // TODO(cmc): This is what I would have normally done, but this never terminates for some
        // reason.
        // let rx = ReceiveSet::new(rxs);
        // while rx.is_connected() {
        //     while let Ok(msg) = rx.recv() {
        //         if let Some(log_msg) = msg.into_data() {
        //             sink.send(log_msg);
        //         }
        //     }
        // }

        sink.flush_blocking();

        Ok(())
    } else {
        #[cfg(feature = "native_viewer")]
        return re_viewer::run_native_app(
            _main_thread_token,
            Box::new(move |cc| {
                let mut app = re_viewer::App::new(
                    _main_thread_token,
                    _build_info,
                    &call_source.app_env(),
                    startup_options,
                    cc.egui_ctx.clone(),
                    cc.storage,
                );
                for rx in rxs {
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
            _ = (call_source, rxs);
            anyhow::bail!(
                "Can't start viewer - rerun was compiled without the 'native_viewer' feature"
            );
        }
    }
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
    } else {
        // 0 means "use all cores", and rayon understands that
        builder = builder.num_threads(threads_args as usize);
    }

    if let Err(err) = builder.build_global() {
        re_log::warn!("Failed to initialize rayon thread pool: {err}");
    }
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
        .ok_or_else(|| anyhow::anyhow!("Invalid size {:?}, expected e.g. 800x600", size))
}

#[cfg(feature = "server")]
fn parse_max_latency(max_latency: Option<&String>) -> f32 {
    max_latency.as_ref().map_or(f32::INFINITY, |time| {
        re_format::parse_duration(time)
            .unwrap_or_else(|err| panic!("Failed to parse max_latency ({max_latency:?}): {err}"))
    })
}

// --- io ---

// TODO(cmc): dedicated module for io utils, especially stdio streaming in and out.

fn stream_to_rrd_on_disk(
    rx: &re_smart_channel::ReceiveSet<LogMsg>,
    path: &std::path::PathBuf,
) -> Result<(), re_log_encoding::FileSinkError> {
    use re_log_encoding::FileSinkError;

    if path.exists() {
        re_log::warn!("Overwriting existing file at {path:?}");
    }

    re_log::info!("Saving incoming log stream to {path:?}. Abort with Ctrl-C.");

    let encoding_options = re_log_encoding::EncodingOptions::MSGPACK_COMPRESSED;
    let file =
        std::fs::File::create(path).map_err(|err| FileSinkError::CreateFile(path.clone(), err))?;
    let mut encoder = re_log_encoding::encoder::DroppableEncoder::new(
        re_build_info::CrateVersion::LOCAL,
        encoding_options,
        file,
    )?;

    loop {
        if let Ok(msg) = rx.recv() {
            if let Some(payload) = msg.into_data() {
                encoder.append(&payload)?;
            }
        } else {
            re_log::info!("Log stream disconnected, stopping.");
            break;
        }
    }

    re_log::info!("File saved to {path:?}");

    Ok(())
}
