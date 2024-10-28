//! Integration with integration with the [`clap`](https://crates.io/crates/clap) command line argument parser.

use std::{net::SocketAddr, path::PathBuf};

use re_sdk::{RecordingStream, RecordingStreamBuilder};

// ---

#[derive(Debug, Clone, PartialEq, Eq)]
enum RerunBehavior {
    Connect(SocketAddr),

    Save(PathBuf),

    Stdout,

    #[cfg(feature = "web_viewer")]
    Serve,

    Spawn,
}

/// This struct implements a `clap::Parser` that defines all the arguments that a typical Rerun
/// application might use, and provides helpers to evaluate those arguments and behave
/// consequently.
///
/// Integrate it into your own `clap::Parser` by flattening it:
/// ```no_run
/// #[derive(Debug, clap::Parser)]
/// #[clap(author, version, about)]
/// struct MyArgs {
///     #[command(flatten)]
///     rerun: rerun::clap::RerunArgs,
///
///     #[clap(long)]
///     my_arg: bool,
/// }
/// ```
///
/// Checkout the official examples to see it used in practice.
#[derive(Clone, Debug, clap::Args)]
#[clap(author, version, about)]
pub struct RerunArgs {
    /// Start a new Rerun Viewer process and feed it data in real-time.
    #[clap(long, default_value = "true")]
    spawn: bool,

    /// Saves the data to an rrd file rather than visualizing it immediately.
    #[clap(long)]
    save: Option<PathBuf>,

    /// Log data to standard output, to be piped into a Rerun Viewer.
    #[clap(long, short = 'o')]
    stdout: bool,

    /// Connects and sends the logged data to a remote Rerun viewer.
    ///
    /// Optionally takes an `ip:port`.
    #[clap(long)]
    #[allow(clippy::option_option)]
    connect: Option<Option<SocketAddr>>,

    /// Connects and sends the logged data to a web-based Rerun viewer.
    #[cfg(feature = "web_viewer")]
    #[clap(long)]
    serve: bool,

    /// An upper limit on how much memory the WebSocket server should use.
    ///
    /// The server buffers log messages for the benefit of late-arriving viewers.
    ///
    /// When this limit is reached, Rerun will drop the oldest data.
    /// Example: `16GB` or `50%` (of system total).
    ///
    /// Defaults to `25%`.
    #[clap(long, default_value = "25%")]
    server_memory_limit: String,

    /// What bind address IP to use.
    #[clap(long, default_value = "0.0.0.0")]
    bind: String,
}

/// [`RerunArgs::init`] might have to spawn a bunch of background tasks depending on what arguments
/// were passed in.
/// This object makes sure they live long enough and get polled as needed.
#[doc(hidden)]
#[derive(Default)]
pub struct ServeGuard {
    block_on_drop: bool,
}

impl Drop for ServeGuard {
    fn drop(&mut self) {
        if self.block_on_drop {
            eprintln!("Sleeping indefinitely while serving web viewer... Press ^C when done.");
            // TODO(andreas): It would be a lot better if we had a handle to the web server and could call `block_until_shutdown` on it.
            std::thread::sleep(std::time::Duration::from_secs(u64::MAX));
        }
    }
}

impl RerunArgs {
    /// Creates a new [`RecordingStream`] according to the CLI parameters.
    #[track_caller] // track_caller so that we can see if we are being called from an official example.
    pub fn init(&self, application_id: &str) -> anyhow::Result<(RecordingStream, ServeGuard)> {
        match self.to_behavior()? {
            RerunBehavior::Stdout => Ok((
                RecordingStreamBuilder::new(application_id).stdout()?,
                Default::default(),
            )),

            RerunBehavior::Connect(addr) => Ok((
                RecordingStreamBuilder::new(application_id)
                    .connect_tcp_opts(addr, crate::default_flush_timeout())?,
                Default::default(),
            )),

            RerunBehavior::Save(path) => Ok((
                RecordingStreamBuilder::new(application_id).save(path)?,
                Default::default(),
            )),

            RerunBehavior::Spawn => Ok((
                RecordingStreamBuilder::new(application_id).spawn()?,
                Default::default(),
            )),

            #[cfg(feature = "web_viewer")]
            RerunBehavior::Serve => {
                let server_memory_limit = re_memory::MemoryLimit::parse(&self.server_memory_limit)
                    .map_err(|err| anyhow::format_err!("Bad --server-memory-limit: {err}"))?;

                let open_browser = true;
                let rec = RecordingStreamBuilder::new(application_id).serve_web(
                    &self.bind,
                    Default::default(),
                    Default::default(),
                    server_memory_limit,
                    open_browser,
                )?;

                // Ensure the server stays alive until the end of the program.
                let sleep_guard = ServeGuard {
                    block_on_drop: true,
                };

                Ok((rec, sleep_guard))
            }
        }
    }

    #[allow(clippy::unnecessary_wraps)] // False positive on some feature flags
    fn to_behavior(&self) -> anyhow::Result<RerunBehavior> {
        if self.stdout {
            return Ok(RerunBehavior::Stdout);
        }

        if let Some(path) = self.save.as_ref() {
            return Ok(RerunBehavior::Save(path.clone()));
        }

        #[cfg(feature = "web_viewer")]
        if self.serve {
            return Ok(RerunBehavior::Serve);
        }

        match self.connect {
            Some(Some(addr)) => return Ok(RerunBehavior::Connect(addr)),
            Some(None) => return Ok(RerunBehavior::Connect(crate::default_server_addr())),
            None => {}
        }

        Ok(RerunBehavior::Spawn)
    }
}
