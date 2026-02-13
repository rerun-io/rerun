/// Options to control the behavior of [`spawn`].
///
/// Refer to the field-level documentation for more information about each individual options.
///
/// The defaults are ok for most use cases: `SpawnOptions::default()`.
/// Use the partial-default pattern to customize them further:
/// ```no_run
/// let opts = re_sdk::SpawnOptions {
///     port: 1234,
///     memory_limit: "25%".into(),
///     ..Default::default()
/// };
/// ```
#[derive(Debug, Clone)]
pub struct SpawnOptions {
    /// The port to listen on.
    ///
    /// Defaults to `9876`.
    pub port: u16,

    /// If `true`, the call to [`spawn`] will block until the Rerun Viewer
    /// has successfully bound to the port.
    pub wait_for_bind: bool,

    /// An upper limit on how much memory the Rerun Viewer should use.
    /// When this limit is reached, Rerun will drop the oldest data.
    /// Example: `16GB` or `50%` (of system total).
    ///
    /// Defaults to `75%`.
    pub memory_limit: String,

    /// An upper limit on how much memory the gRPC server running
    /// in the same process as the Rerun Viewer should use.
    /// When this limit is reached, Rerun will drop the oldest data.
    /// Example: `16GB` or `50%` (of system total).
    ///
    /// Defaults to `1GiB`.
    pub server_memory_limit: String,

    /// Specifies the name of the Rerun executable.
    ///
    /// You can omit the `.exe` suffix on Windows.
    ///
    /// Defaults to `rerun`.
    pub executable_name: String,

    /// Enforce a specific executable to use instead of searching though PATH
    /// for [`Self::executable_name`].
    ///
    /// Unspecified by default.
    pub executable_path: Option<String>,

    /// Extra arguments that will be passed as-is to the Rerun Viewer process.
    pub extra_args: Vec<String>,

    /// Extra environment variables that will be passed as-is to the Rerun Viewer process.
    pub extra_env: Vec<(String, String)>,

    /// Hide the welcome screen.
    pub hide_welcome_screen: bool,

    /// Detach Rerun Viewer process from the application process.
    pub detach_process: bool,
}

// NOTE: No need for .exe extension on windows.
const RERUN_BINARY: &str = "rerun";

impl Default for SpawnOptions {
    fn default() -> Self {
        Self {
            port: re_grpc_server::DEFAULT_SERVER_PORT,
            wait_for_bind: false,
            memory_limit: "75%".into(),
            server_memory_limit: "1GiB".into(),
            executable_name: RERUN_BINARY.into(),
            executable_path: None,
            extra_args: Vec::new(),
            extra_env: Vec::new(),
            hide_welcome_screen: false,
            detach_process: true,
        }
    }
}

impl SpawnOptions {
    /// Resolves the final connect address value.
    pub fn connect_addr(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            self.port,
        )
    }

    /// Resolves the final listen address value.
    pub fn listen_addr(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            self.port,
        )
    }

    /// Resolves the final executable path.
    pub fn executable_path(&self) -> String {
        if let Some(path) = self.executable_path.as_deref() {
            return path.to_owned();
        }

        #[cfg(debug_assertions)]
        {
            let cargo_target_dir =
                std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_owned());
            let local_build_path = format!(
                "{cargo_target_dir}/debug/{}{}",
                self.executable_name,
                std::env::consts::EXE_SUFFIX
            );
            if std::fs::metadata(&local_build_path).is_ok() {
                re_log::info!("Spawning the locally built rerun at {local_build_path}");
                return local_build_path;
            } else {
                re_log::info!(
                    "No locally built rerun found at {local_build_path:?}, using executable named {:?} from PATH.",
                    self.executable_name
                );
            }
        }

        self.executable_name.clone()
    }
}

/// Errors that can occur when [`spawn`]ing a Rerun Viewer.
#[derive(thiserror::Error)]
pub enum SpawnError {
    /// Failed to find Rerun Viewer executable in PATH.
    #[error("Failed to find Rerun Viewer executable in PATH.\n{message}\nPATH={search_path:?}")]
    ExecutableNotFoundInPath {
        /// High-level error message meant to be printed to the user (install tips etc).
        message: String,

        /// Name used for the executable search.
        executable_name: String,

        /// Value of the `PATH` environment variable, if any.
        search_path: String,
    },

    /// Failed to find Rerun Viewer executable at explicit path.
    #[error("Failed to find Rerun Viewer executable at {executable_path:?}")]
    ExecutableNotFound {
        /// Explicit path of the executable (specified by the caller).
        executable_path: String,
    },

    /// Other I/O error.
    #[error("Failed to spawn the Rerun Viewer process: {0}")]
    Io(#[from] std::io::Error),
}

impl std::fmt::Debug for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Due to how recording streams are initialized in practice, most of the time `SpawnError`s
        // will bubble all the way up to `main` and crash the program, which will call into the
        // `Debug` implementation.
        //
        // Spawn errors include a user guide, and so we need them to render in a nice way.
        // Hence we redirect the debug impl to the display impl generated by `thiserror`.
        <Self as std::fmt::Display>::fmt(self, f)
    }
}

/// Spawns a new Rerun Viewer process ready to listen for connections.
///
/// If there is already a process listening on this port (Rerun or not), this function returns `Ok`
/// WITHOUT spawning a `rerun` process (!).
///
/// Refer to [`SpawnOptions`]'s documentation for configuration options.
///
/// This only starts a Viewer process: if you'd like to connect to it and start sending data, refer
/// to [`crate::RecordingStream::connect_grpc`] or use [`crate::RecordingStream::spawn`] directly.
pub fn spawn(opts: &SpawnOptions) -> Result<(), SpawnError> {
    use std::net::TcpStream;
    #[cfg(target_family = "unix")]
    use std::os::unix::process::CommandExt as _;
    use std::process::Command;
    use std::time::Duration;

    // NOTE: These are indented on purpose, it just looks better and reads easier.

    const MSG_INSTALL_HOW_TO: &str = //
    "
    You can install binary releases of the Rerun Viewer:
    * Using `cargo`: `cargo binstall rerun-cli` (see https://github.com/cargo-bins/cargo-binstall)
    * Via direct download from our release assets: https://github.com/rerun-io/rerun/releases/latest/
    * Using `pip`: `pip3 install rerun-sdk`

    For more information, refer to our complete install documentation over at:
    https://rerun.io/docs/getting-started/installing-viewer
    ";

    const MSG_INSTALL_HOW_TO_VERSIONED: &str = //
    "
    You can install an appropriate version of the Rerun Viewer via binary releases:
    * Using `cargo`: `cargo binstall --force rerun-cli@__VIEWER_VERSION__` (see https://github.com/cargo-bins/cargo-binstall)
    * Via direct download from our release assets: https://github.com/rerun-io/rerun/releases/__VIEWER_VERSION__/
    * Using `pip`: `pip3 install rerun-sdk==__VIEWER_VERSION__`

    For more information, refer to our complete install documentation over at:
    https://rerun.io/docs/getting-started/installing-viewer
    ";

    const MSG_VERSION_MISMATCH: &str = //
        "
    ⚠ The version of the Rerun Viewer available on your PATH does not match the version of your Rerun SDK ⚠

    Rerun does not make any kind of backwards/forwards compatibility guarantee yet: this can lead to (subtle) bugs.

    > Rerun Viewer: v__VIEWER_VERSION__ (executable: \"__VIEWER_PATH__\")
    > Rerun SDK: v__SDK_VERSION__";

    let port = opts.port;
    let connect_addr = opts.connect_addr();
    let memory_limit = &opts.memory_limit;
    let server_memory_limit = &opts.server_memory_limit;
    let executable_path = opts.executable_path();

    // TODO(#4019): application-level handshake
    if TcpStream::connect_timeout(&connect_addr, Duration::from_secs(1)).is_ok() {
        re_log::info!(
            addr = %opts.listen_addr(),
            "A process is already listening at this address. Assuming it's a Rerun Viewer."
        );
        return Ok(());
    }

    let map_err = |err: std::io::Error| -> SpawnError {
        if err.kind() == std::io::ErrorKind::NotFound {
            if let Some(executable_path) = opts.executable_path.as_ref() {
                SpawnError::ExecutableNotFound {
                    executable_path: executable_path.clone(),
                }
            } else {
                let sdk_version = re_build_info::build_info!().version;
                SpawnError::ExecutableNotFoundInPath {
                    // Only recommend a specific Viewer version for non-alpha/rc/dev SDKs.
                    message: if sdk_version.is_release() {
                        MSG_INSTALL_HOW_TO_VERSIONED
                            .replace("__VIEWER_VERSION__", &sdk_version.to_string())
                    } else {
                        MSG_INSTALL_HOW_TO.to_owned()
                    },
                    executable_name: opts.executable_name.clone(),
                    search_path: std::env::var("PATH").unwrap_or_else(|_| String::new()),
                }
            }
        } else {
            err.into()
        }
    };

    // Try to check the version of the Viewer.
    // Do not fail if we can't retrieve the version, it's not a critical error.
    let viewer_version = Command::new(&executable_path)
        .arg("--version")
        .output()
        .ok()
        .and_then(|output| {
            let output = String::from_utf8_lossy(&output.stdout);
            re_build_info::CrateVersion::try_parse_from_build_info_string(output).ok()
        });

    if let Some(viewer_version) = viewer_version {
        let sdk_version = re_build_info::build_info!().version;

        if !viewer_version.is_compatible_with(sdk_version) {
            eprintln!(
                "{}",
                MSG_VERSION_MISMATCH
                    .replace("__VIEWER_VERSION__", &viewer_version.to_string())
                    .replace("__VIEWER_PATH__", &executable_path)
                    .replace("__SDK_VERSION__", &sdk_version.to_string())
            );

            // Don't recommend installing stuff through registries if the user is running some
            // weird version.
            if sdk_version.is_release() {
                eprintln!(
                    "{}",
                    MSG_INSTALL_HOW_TO_VERSIONED
                        .replace("__VIEWER_VERSION__", &sdk_version.to_string())
                );
            } else {
                eprintln!();
            }
        }
    }

    let mut rerun_bin = Command::new(&executable_path);

    // By default stdin is inherited which may cause issues in some debugger setups.
    // Also, there's really no reason to forward stdin to the child process in this case.
    // `stdout`/`stderr` we leave at default inheritance because it can be useful to see the Viewer's output.
    rerun_bin
        .stdin(std::process::Stdio::null())
        .arg(format!("--port={port}"))
        .arg(format!("--memory-limit={memory_limit}"))
        .arg(format!("--server-memory-limit={server_memory_limit}"))
        .arg("--expect-data-soon");

    if opts.hide_welcome_screen {
        rerun_bin.arg("--hide-welcome-screen");
    }

    rerun_bin.args(opts.extra_args.clone());
    rerun_bin.envs(opts.extra_env.clone());

    if opts.detach_process {
        // SAFETY: This code is only run in the child fork, we are not modifying any memory
        // that is shared with the parent process.
        #[cfg(target_family = "unix")]
        #[expect(unsafe_code)]
        unsafe {
            rerun_bin.pre_exec(|| {
                // On unix systems, we want to make sure that the child process becomes its
                // own session leader, so that it doesn't die if the parent process crashes
                // or is killed.
                libc::setsid();

                Ok(())
            })
        };
    }

    rerun_bin.spawn().map_err(map_err)?;

    if opts.wait_for_bind {
        // Give the newly spawned Rerun Viewer some time to bind.
        //
        // NOTE: The timeout only covers the TCP handshake: if no process is bound to that address
        // at all, the connection will fail immediately, irrelevant of the timeout configuration.
        // For that reason we use an extra loop.
        for i in 0..5 {
            re_log::debug!("connection attempt {}", i + 1);
            if TcpStream::connect_timeout(&connect_addr, Duration::from_secs(1)).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    // Simply forget about the child process, we want it to outlive the parent process if needed.
    _ = rerun_bin;

    Ok(())
}
