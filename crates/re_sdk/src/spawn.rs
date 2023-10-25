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

    /// An upper limit on how much memory the Rerun Viewer should use.
    /// When this limit is reached, Rerun will drop the oldest data.
    /// Example: `16GB` or `50%` (of system total).
    ///
    /// Defaults to `75%`.
    pub memory_limit: String,

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
}

// NOTE: No need for .exe extension on windows.
const RERUN_BINARY: &str = "rerun";

impl Default for SpawnOptions {
    fn default() -> Self {
        Self {
            port: crate::default_server_addr().port(),
            memory_limit: "75%".into(),
            executable_name: RERUN_BINARY.into(),
            executable_path: None,
        }
    }
}

impl SpawnOptions {
    /// Resolves the final connect address value.
    pub fn connect_addr(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::new("127.0.0.1".parse().unwrap(), self.port)
    }

    /// Resolves the final listen address value.
    pub fn listen_addr(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::new("0.0.0.0".parse().unwrap(), self.port)
    }

    /// Resolves the final executable path.
    pub fn executable_path(&self) -> String {
        if let Some(path) = self.executable_path.as_deref() {
            return path.to_owned();
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

/// Spawns a new Rerun Viewer process ready to listen for TCP connections.
///
/// Refer to [`SpawnOptions`]'s documentation for configuration options.
///
/// This only starts a Viewer process: if you'd like to connect to it and start sending data, refer
/// to [`crate::RecordingStream::connect`] or use [`crate::RecordingStream::spawn`] directly.
pub fn spawn(opts: &SpawnOptions) -> Result<(), SpawnError> {
    use std::{net::TcpStream, process::Command, time::Duration};

    // NOTE: It's indented on purpose, it just looks better and reads easier.
    const EXECUTABLE_NOT_FOUND: &str = //
    "
    You can install binary releases of the Rerun Viewer:
    * Using `cargo`: `cargo binstall rerun-cli` (see https://github.com/cargo-bins/cargo-binstall)
    * Via direct download from our release assets: https://github.com/rerun-io/rerun/releases/latest/
    * Using `pip`: `pip3 install rerun-sdk` (warning: pip version has slower start times!)

    For more information, refer to our complete install documentation over at:
    https://rerun.io/docs/getting-started/installing-viewer
    ";

    let port = opts.port;
    let connect_addr = opts.connect_addr();
    let memory_limit = &opts.memory_limit;
    let executable_path = opts.executable_path();

    if TcpStream::connect_timeout(&connect_addr, Duration::from_millis(1)).is_ok() {
        re_log::info!(
            addr = %opts.listen_addr(),
            "A process is already listening at this address. Assuming it's a Rerun Viewer."
        );
        return Ok(());
    }

    let res = Command::new(executable_path)
        .arg(format!("--port={port}"))
        .arg(format!("--memory-limit={memory_limit}"))
        .arg("--skip-welcome-screen")
        .spawn();

    let rerun_bin = match res {
        Ok(rerun_bin) => rerun_bin,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return if let Some(executable_path) = opts.executable_path.as_ref() {
                Err(SpawnError::ExecutableNotFound {
                    executable_path: executable_path.clone(),
                })
            } else {
                Err(SpawnError::ExecutableNotFoundInPath {
                    message: EXECUTABLE_NOT_FOUND.to_owned(),
                    executable_name: opts.executable_name.clone(),
                    search_path: std::env::var("PATH").unwrap_or_else(|_| String::new()),
                })
            }
        }
        Err(err) => {
            return Err(err.into());
        }
    };

    // Simply forget about the child process, we want it to outlive the parent process if needed.
    _ = rerun_bin;

    Ok(())
}
