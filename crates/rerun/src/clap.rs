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
#[derive(Debug, clap::Args, Clone)]
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
    /// Run common Rerun script setup actions. Connect to the viewer if necessary.
    ///
    /// Returns `true` if you should call `session.spawn`.
    pub fn on_startup(&self, session: &mut Session) -> anyhow::Result<bool> {
        match self.to_behavior() {
            RerunBehavior::Connect(addr) => session.connect(addr),

            RerunBehavior::Save(path) => session.save(path)?,

            #[cfg(feature = "web_viewer")]
            RerunBehavior::Serve => {
                crate::serve_web_viewer(session, true);
            }

            RerunBehavior::Spawn => return Ok(true),
        }

        Ok(false)
    }

    /// Run common post-actions. Sleep if serving the web viewer.
    pub fn on_teardown(&self) {
        #[cfg(feature = "web_viewer")]
        if self.to_behavior() == RerunBehavior::Serve {
            eprintln!("Sleeping while serving the web viewer. Abort with Ctrl-C"); // TODO(emilk): sleep in `drop` instead?
            std::thread::sleep(std::time::Duration::from_secs(1_000_000));
        }
    }

    fn to_behavior(&self) -> RerunBehavior {
        if let Some(path) = self.save.as_ref() {
            return RerunBehavior::Save(path.clone());
        }

        #[cfg(feature = "web_viewer")]
        if self.serve {
            return RerunBehavior::Serve;
        }

        match self.connect {
            Some(Some(addr)) => return RerunBehavior::Connect(addr),
            Some(None) => return RerunBehavior::Connect(crate::default_server_addr()),
            None => {}
        }

        RerunBehavior::Spawn
    }
}
