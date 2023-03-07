//! This module provides integration with integration with [`clap`](https://github.com/clap-rs/clap).

use std::{net::SocketAddr, path::PathBuf};

use crate::Session;

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
}

impl RerunArgs {
    /// Set up Rerun, and run the given code with a [`Session`] object
    /// that can be used to log data.
    ///
    /// Logging will be controlled by the `RERUN` environment variable,
    /// or the `default_enabled` argument if the environment variable is not set.
    pub fn run(
        &self,
        application_id: &str,
        default_enabled: bool,
        run: impl FnOnce(Session) + Send + 'static,
    ) -> anyhow::Result<()> {
        let mut session = Session::init(application_id, default_enabled);

        match self.to_behavior()? {
            RerunBehavior::Connect(addr) => {
                session.connect(addr);
                run(session);
            }

            RerunBehavior::Save(path) => {
                session.save(path)?;
                run(session);
            }

            #[cfg(feature = "web_viewer")]
            RerunBehavior::Serve => {
                crate::web_viewer::serve(&mut session, true);
                run(session);
                eprintln!("Sleeping while serving the web viewer. Abort with Ctrl-C");
                std::thread::sleep(std::time::Duration::from_secs(1_000_000));
            }

            #[cfg(feature = "native_viewer")]
            RerunBehavior::Spawn => {
                crate::native_viewer::spawn(session, run)?;
            }
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
