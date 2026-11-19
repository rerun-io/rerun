//! `rerun mcp`: an MCP server for agents, see [`re_rerun_mcp`].

/// The arguments of `rerun mcp`.
#[derive(Debug, Clone, clap::Parser)]
pub struct McpCommand {
    /// gRPC endpoint of the viewer to connect to on startup, e.g. `http://127.0.0.1:9876`.
    ///
    /// Without it, the server starts unconnected and the agent picks a viewer with its
    /// `viewer_connect` tool.
    #[arg(long)]
    endpoint: Option<url::Url>,

    /// URL of the catalog server for the catalog tools, e.g. `rerun+http://127.0.0.1:51234`.
    ///
    /// Without it, they act on the catalog of the connected viewer if it runs on this
    /// machine, until the agent picks another one with its `catalog_connect` tool.
    #[arg(long, value_parser = re_catalog_tools::CatalogTools::parse_url)]
    catalog_url: Option<re_uri::Origin>,

    /// The toolsets to offer, comma-separated.
    #[arg(long, value_enum, value_delimiter = ',', default_values_t = [McpToolset::Viewer, McpToolset::Catalog])]
    toolsets: Vec<McpToolset>,

    // TODO(eleijonmarck): find a cleaner way for analytics to know who started the server.
    /// Marks a server started by the viewer's agent panel.
    ///
    /// Only analytics reads it, the server behaves the same either way.
    #[arg(long, hide = true)]
    pub from_agent_panel: bool,
}

/// A group of tools `rerun mcp` can offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum McpToolset {
    /// Drive a running Rerun Viewer.
    Viewer,

    /// Read a Rerun catalog server.
    Catalog,
}

impl McpCommand {
    /// Serve MCP over stdio until the agent disconnects.
    pub fn run(self, tokio_runtime: &tokio::runtime::Runtime) -> anyhow::Result<()> {
        let config = self.config()?;
        tokio_runtime.block_on(re_rerun_mcp::serve(config))?;
        Ok(())
    }

    /// The MCP server's configuration, refusing an endpoint for a toolset that is left out.
    fn config(self) -> anyhow::Result<re_rerun_mcp::Config> {
        let Self {
            endpoint,
            catalog_url,
            toolsets,
            from_agent_panel: _,
        } = self;
        let has_viewer = toolsets.contains(&McpToolset::Viewer);
        let has_catalog = toolsets.contains(&McpToolset::Catalog);
        if endpoint.is_some() && !has_viewer {
            anyhow::bail!("`--endpoint` needs the viewer toolset, see `--toolsets`");
        }
        if catalog_url.is_some() && !has_catalog {
            anyhow::bail!("`--catalog-url` needs the catalog toolset, see `--toolsets`");
        }
        Ok(re_rerun_mcp::Config {
            viewer: has_viewer.then_some(re_rerun_mcp::ViewerConfig { endpoint }),
            catalog: has_catalog.then_some(re_rerun_mcp::CatalogConfig {
                origin: catalog_url,
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::*;

    fn parse(args: &[&str]) -> McpCommand {
        McpCommand::try_parse_from(std::iter::chain(
            std::iter::once("mcp"),
            args.iter().copied(),
        ))
        .expect("valid arguments")
    }

    #[test]
    fn from_agent_panel_is_off_by_default() {
        assert!(!parse(&[]).from_agent_panel);
        assert!(parse(&["--from-agent-panel"]).from_agent_panel);
    }

    #[test]
    fn every_toolset_is_offered_by_default() {
        assert_eq!(
            parse(&[]).toolsets,
            [McpToolset::Viewer, McpToolset::Catalog]
        );
        assert_eq!(
            parse(&["--toolsets", "catalog"]).toolsets,
            [McpToolset::Catalog]
        );
        assert_eq!(
            parse(&["--toolsets", "viewer,catalog"]).toolsets,
            [McpToolset::Viewer, McpToolset::Catalog]
        );
    }

    #[test]
    fn endpoints_need_their_toolset() {
        let err = parse(&[
            "--toolsets",
            "catalog",
            "--endpoint",
            "http://127.0.0.1:9876",
        ])
        .config()
        .expect_err("no viewer toolset");
        assert!(err.to_string().contains("--endpoint"), "{err}");
        let err = parse(&[
            "--toolsets",
            "viewer",
            "--catalog-url",
            "http://127.0.0.1:51234",
        ])
        .config()
        .expect_err("no catalog toolset");
        assert!(err.to_string().contains("--catalog-url"), "{err}");

        let config = parse(&[
            "--endpoint",
            "http://127.0.0.1:9876",
            "--catalog-url",
            "http://127.0.0.1:51234",
        ])
        .config()
        .expect("valid");
        assert!(
            config
                .viewer
                .is_some_and(|viewer| viewer.endpoint.is_some())
        );
        assert!(
            config
                .catalog
                .is_some_and(|catalog| catalog.origin.is_some())
        );
        let config = parse(&["--toolsets", "catalog"]).config().expect("valid");
        assert!(config.viewer.is_none());
    }
}
