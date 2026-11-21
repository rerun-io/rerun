//! `rerun servers`: the catalog servers the viewer, the CLI, and `rerun mcp` know about.

use re_redap_client::KnownServers;

/// Manage the catalog servers that Rerun remembers.
#[derive(Debug, Clone, clap::Subcommand)]
pub enum ServersCommands {
    /// List the catalog servers Rerun remembers.
    List,

    /// Remember a catalog server, so `rerun mcp` offers it to agents, and the Viewer shows it the
    /// next time it starts.
    Add {
        /// The server's URL, e.g. `rerun+https://example.com` or `http://127.0.0.1:51234`.
        #[arg(value_parser = re_uri::Origin::from_user_input)]
        url: re_uri::Origin,
    },

    /// Forget a catalog server.
    ///
    /// A running Viewer keeps showing it until it restarts. A login the Viewer saved for it stays
    /// in the Viewer's settings; remove the server in the Viewer instead to drop that too.
    Remove {
        /// The server's URL, as `rerun servers list` shows it.
        #[arg(value_parser = re_uri::Origin::from_user_input)]
        url: re_uri::Origin,
    },
}

impl ServersCommands {
    /// Run on the server list in Rerun's config directory, writing a listing to stdout.
    pub fn run(self) -> anyhow::Result<()> {
        let Some(known) = KnownServers::in_config_dir() else {
            anyhow::bail!("Could not find Rerun's config directory: there is no home directory");
        };
        self.run_on(&known, &mut std::io::stdout().lock())
    }

    /// Run on the list `known`, writing a listing to `out`.
    fn run_on(self, known: &KnownServers, out: &mut impl std::io::Write) -> anyhow::Result<()> {
        match self {
            Self::List => {
                for origin in known.load()? {
                    writeln!(out, "{origin}")?;
                }
            }
            Self::Add { url } => {
                if known.add(&url)? {
                    re_log::info!("Added {url}");
                } else {
                    re_log::info!("{url} was already known");
                }
            }
            Self::Remove { url } => {
                if known.remove(&url)? {
                    re_log::info!("Removed {url}");
                } else {
                    anyhow::bail!("Not a known server, see `rerun servers list`: {url}");
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_list() -> (KnownServers, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("temp dir");
        (KnownServers::at(dir.path().join("servers.json")), dir)
    }

    fn url() -> re_uri::Origin {
        re_uri::Origin::from_user_input("http://127.0.0.1:51234").expect("valid url")
    }

    #[test]
    fn list_prints_one_url_per_line() {
        let (known, _dir) = temp_list();
        ServersCommands::Add { url: url() }
            .run_on(&known, &mut std::io::sink())
            .expect("add");
        let mut out = Vec::new();
        ServersCommands::List
            .run_on(&known, &mut out)
            .expect("list");
        assert_eq!(
            String::from_utf8(out).expect("utf8"),
            "rerun+http://127.0.0.1:51234\n"
        );
    }

    #[test]
    fn adding_a_known_server_again_adds_nothing() {
        let (known, _dir) = temp_list();
        for _ in 0..2 {
            ServersCommands::Add { url: url() }
                .run_on(&known, &mut std::io::sink())
                .expect("add");
        }
        assert_eq!(known.load().expect("load"), [url()]);
    }

    #[test]
    fn a_server_added_as_http_is_removed_as_rerun_http() {
        let (known, _dir) = temp_list();
        ServersCommands::Add { url: url() }
            .run_on(&known, &mut std::io::sink())
            .expect("add");
        let url = re_uri::Origin::from_user_input("rerun+http://127.0.0.1:51234").expect("url");
        ServersCommands::Remove { url }
            .run_on(&known, &mut std::io::sink())
            .expect("remove");
        assert_eq!(known.load().expect("load"), []);
    }

    #[test]
    fn removing_an_unknown_server_is_an_error() {
        let (known, _dir) = temp_list();
        let err = ServersCommands::Remove { url: url() }
            .run_on(&known, &mut std::io::sink())
            .expect_err("not known");
        assert_eq!(
            err.to_string(),
            "Not a known server, see `rerun servers list`: rerun+http://127.0.0.1:51234"
        );
    }
}
