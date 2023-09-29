//! Integration with integration with the [`clap`](https://crates.io/crates/clap) command line argument parser.

use std::{net::SocketAddr, path::PathBuf};

use re_sdk::RecordingStream;

// ---

#[derive(Debug, Clone, PartialEq, Eq)]
enum RerunBehavior {
    Connect(SocketAddr),

    Save(PathBuf),

    #[cfg(feature = "web_viewer")]
    Serve,

    #[cfg(feature = "native_viewer")]
    Spawn,
}

// TODO(cmc): There are definitely ways of making this all nicer now (this, native_viewer and
// web_viewer).. but one thing at a time.

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
    /// Start a viewer and feed it data in real-time.
    #[clap(long, default_value = "true")]
    spawn: bool,

    /// Saves the data to an rrd file rather than visualizing it immediately.
    #[clap(long)]
    save: Option<PathBuf>,

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

    /// What bind address IP to use.
    #[clap(long, default_value = "0.0.0.0")]
    bind: String,
}

impl RerunArgs {
    /// Set up Rerun, and run the given code with a [`RecordingStream`] object
    /// that can be used to log data.
    ///
    /// Logging will be controlled by the `RERUN` environment variable,
    /// or the `default_enabled` argument if the environment variable is not set.
    #[track_caller] // track_caller so that we can see if we are being called from an official example.
    pub fn run(
        &self,
        application_id: &str,
        default_enabled: bool,
        run: impl FnOnce(RecordingStream) + Send + 'static,
    ) -> anyhow::Result<()> {
        // Ensure we have a running tokio runtime.
        let mut tokio_runtime = None;
        let tokio_runtime_handle = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle
        } else {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            tokio_runtime.get_or_insert(rt).handle().clone()
        };
        let _tokio_runtime_guard = tokio_runtime_handle.enter();

        let (rerun_enabled, store_info, batcher_config) =
            crate::RecordingStreamBuilder::new(application_id)
                .default_enabled(default_enabled)
                .into_args();

        if !rerun_enabled {
            run(RecordingStream::disabled());
            return Ok(());
        }

        let sink: Box<dyn re_sdk::sink::LogSink> = match self.to_behavior()? {
            RerunBehavior::Connect(addr) => Box::new(crate::sink::TcpSink::new(
                addr,
                crate::default_flush_timeout(),
            )),

            RerunBehavior::Save(path) => Box::new(crate::sink::FileSink::new(path)?),

            #[cfg(feature = "web_viewer")]
            RerunBehavior::Serve => {
                let open_browser = true;
                re_sdk::web_viewer::new_sink(
                    open_browser,
                    &self.bind,
                    Default::default(),
                    Default::default(),
                )?
            }

            #[cfg(feature = "native_viewer")]
            RerunBehavior::Spawn => {
                crate::native_viewer::spawn(store_info, batcher_config, run)?;
                return Ok(());
            }
        };

        let rec = RecordingStream::new(store_info, batcher_config, sink)?;
        run(rec.clone());

        // The user callback is done executing, it's a good opportunity to flush the pipeline
        // independently of the current flush thresholds (which might be `NEVER`).
        rec.flush_async();

        #[cfg(feature = "web_viewer")]
        if matches!(self.to_behavior(), Ok(RerunBehavior::Serve)) {
            // Sleep waiting for Ctrl-C:
            tokio_runtime_handle.block_on(async {
                tokio::time::sleep(std::time::Duration::from_secs(u64::MAX)).await;
            });
        }

        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)] // False positive on some feature flags
    fn to_behavior(&self) -> anyhow::Result<RerunBehavior> {
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

        #[cfg(not(feature = "native_viewer"))]
        anyhow::bail!("Expected --save, --connect, or --serve");

        #[cfg(feature = "native_viewer")]
        Ok(RerunBehavior::Spawn)
    }
}
