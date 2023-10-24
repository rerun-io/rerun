/// Options to control the behavior of [`spawn`].
///
/// Refer to the field-level documentation for more information about each individual options.
///
/// The defaults are ok for most use cases: `SpawnOptions::default()`.
/// Use the builder pattern to customize them further:
/// ```no_run
/// let opts = re_sdk::SpawnOptions::default().with_port(1234u16).with_memory_limit("25%");
/// ```
#[derive(Debug, Clone, Default)]
pub struct SpawnOptions {
    /// The port to listen on.
    ///
    /// Defaults to `9876` if unspecified.
    pub port: Option<u16>,

    /// An upper limit on how much memory the Rerun Viewer should use.
    /// When this limit is reached, Rerun will drop the oldest data.
    /// Example: `16GB` or `50%` (of system total).
    ///
    /// Defaults to `75%` if unspecified.
    pub memory_limit: Option<String>,

    /// Specifies the name of the Rerun executable.
    ///
    /// You can omit the `.exe` suffix on Windows.
    ///
    /// Defaults to `rerun` if unspecified.
    pub executable_name: Option<String>,

    /// Enforce a specific executable to use instead of searching though PATH
    /// for [`Self::executable_name`].
    ///
    /// Unspecified by default.
    pub executable_path: Option<String>,
}

impl SpawnOptions {
    /// Refer to field-level documentation.
    pub fn with_port(mut self, port: impl Into<u16>) -> Self {
        self.port = Some(port.into());
        self
    }

    /// Refer to field-level documentation.
    pub fn with_memory_limit(mut self, memory_limit: impl AsRef<str>) -> Self {
        self.memory_limit = Some(memory_limit.as_ref().to_owned());
        self
    }

    /// Refer to field-level documentation.
    pub fn with_executable_name(mut self, executable_name: impl AsRef<str>) -> Self {
        self.executable_name = Some(executable_name.as_ref().to_owned());
        self
    }

    /// Refer to field-level documentation.
    pub fn with_executable_path(mut self, executable_path: impl AsRef<str>) -> Self {
        self.executable_path = Some(executable_path.as_ref().to_owned());
        self
    }
}

impl SpawnOptions {
    /// Resolves the final port value.
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(9876)
    }

    /// Resolves the final connect address value.
    pub fn connect_addr(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::new("127.0.0.1".parse().unwrap(), self.port())
    }

    /// Resolves the final listen address value.
    pub fn listen_addr(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::new("0.0.0.0".parse().unwrap(), self.port())
    }

    /// Resolves the final memory limit value.
    pub fn memory_limit(&self) -> String {
        self.memory_limit.as_deref().unwrap_or("75%").to_owned()
    }

    /// Resolves the final executable path.
    pub fn executable_path(&self) -> String {
        // NOTE: No need for .exe extension on windows.
        const RERUN_BINARY: &str = "rerun";

        if let Some(path) = self.executable_path.as_deref() {
            return path.to_owned();
        }

        self.executable_name
            .as_deref()
            .unwrap_or(RERUN_BINARY)
            .to_owned()
    }
}

/// Spawns a new Rerun Viewer process ready to listen for TCP connections.
///
/// Refer to [`SpawnOptions`]'s documentation for configuration options.
///
/// This only starts a Viewer process: if you'd like to connect to it and start sending data, refer
/// to [`crate::RecordingStream::connect`] or use [`crate::RecordingStream::spawn`] directly.
pub fn spawn(opts: &SpawnOptions) -> std::io::Result<()> {
    use std::{net::TcpStream, process::Command, time::Duration};

    // NOTE: It's indented on purpose, it just looks better and reads easier.
    const EXECUTABLE_NOT_FOUND: &str = //
    "
    Couldn't find the Rerun Viewer executable in your PATH.

    You can install binary releases of the Rerun Viewer using any of the following methods:
    * Binary download with `cargo`: `cargo binstall rerun-cli` (see https://github.com/cargo-bins/cargo-binstall)
    * Build from source with `cargo`: `cargo install rerun-cli` (requires Rust 1.72+)
    * Direct download from our release assets: https://github.com/rerun-io/rerun/releases/latest/
    * Or together with the Rerun Python SDK:
      * Pip: `pip3 install rerun-sdk`
      * Conda: `conda install -c conda-forge rerun-sdk`
      * Binary download with `pixi`: `pixi global install rerun-sdk` (see https://prefix.dev/docs/pixi/overview)

    If your platform and/or architecture is not available, you can refer to
    https://github.com/rerun-io/rerun/blob/main/BUILD.md for instructions on how to build from source.

    Otherwise, feel free to open an issue at https://github.com/rerun-io/rerun/issues if you'd like to
    request binary releases for your specific platform.
    ";

    let port = opts.port();
    let connect_addr = opts.connect_addr();
    let memory_limit = opts.memory_limit();
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
            eprintln!("{EXECUTABLE_NOT_FOUND}");
            return Err(err);
        }
        Err(err) => {
            re_log::info!(%err, "Failed to spawn Rerun Viewer");
            return Err(err);
        }
    };

    // Simply forget about the child process, we want it to outlive the parent process if needed.
    _ = rerun_bin;

    Ok(())
}
