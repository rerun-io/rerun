//! The catalog toolset: the `re_catalog_tools` toolbox, served over MCP.

use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};

use rmcp::model::{CallToolRequestParams, CallToolResult, Tool};

use re_agent_tools::CatalogAccess;
use re_catalog_tools::{CatalogTool, CatalogTools};

use crate::{ServeError, toolbox};

/// Operating guidance for the catalog tools, part of the MCP `instructions`.
const INSTRUCTIONS: &str = include_str!("catalog_instructions.md");

/// The catalog toolset, connected to at most one catalog server at a time.
///
/// A catalog server (`rerun server`, Rerun Hub, or the viewer's own) has exactly one catalog.
pub struct CatalogToolset {
    tools: CatalogTools,

    /// See [`Self::defaults_to_viewer_catalog`].
    defaults_to_viewer_catalog: AtomicBool,
}

impl CatalogToolset {
    /// Must be called inside the Tokio runtime the server runs on.
    pub fn new(origin: Option<re_uri::Origin>, access: CatalogAccess) -> Result<Self, ServeError> {
        let runtime = re_async::AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()
            .map_err(|source| ServeError::NoRuntime { source })?;
        let registry = re_redap_client::ConnectionRegistry::new_with_stored_credentials()
            .with_client_name("rerun-mcp".to_owned());
        let defaults_to_viewer_catalog = AtomicBool::new(origin.is_none());
        let tools = CatalogTools::new(registry, runtime, origin)
            .with_known_servers(re_redap_client::KnownServers::in_config_dir());
        tools.set_access(access);
        Ok(Self {
            tools,
            defaults_to_viewer_catalog,
        })
    }

    /// Whether the agent's tool `name` is one of this toolset's.
    pub fn owns(name: &str) -> bool {
        CatalogTool::from_name(name).is_some()
    }

    /// Every tool of this toolset, as the agent sees it.
    pub fn tools(&self) -> Vec<Tool> {
        toolbox::to_mcp_tools(&self.tools)
    }

    /// The catalog part of the MCP `instructions`.
    ///
    /// `with_viewer` says whether the server also has the viewer toolset, whose catalog the
    /// catalog tools fall back to.
    pub fn instructions(&self, with_viewer: bool) -> String {
        let connect = CatalogTool::Connect.name();
        // A catalog chosen up front wins over the viewer's.
        let chosen_catalog = if self.defaults_to_viewer_catalog() {
            None
        } else {
            self.tools.origin()
        };
        let start = if let Some(origin) = chosen_catalog {
            format!(
                "The catalog tools start out acting on the catalog at `{origin}`. \
                 Call `{connect}` to read another server."
            )
        } else if with_viewer {
            format!(
                "Until you call `{connect}`, the catalog tools act on the catalog of the \
                 connected viewer (its `catalog_url`) when that viewer runs on this machine. \
                 It holds the recordings the viewer loaded from files: read their values with \
                 `{query}` rather than the Python API. Call `{connect}` to read another server, \
                 e.g. one from the `servers` that `viewer_get_state` lists.\n\
                 - A recording's `store_id` in `viewer_get_state` names its dataset id and \
                 segment id: pass them as `dataset` and `segment_id`.",
                query = CatalogTool::QueryDataset.name(),
            )
        } else {
            format!("Call `{connect}` with the catalog's URL first.")
        };
        let mut instructions = format!("{}\n- {start}", INSTRUCTIONS.trim_end());
        if self.tools.access().allows_writes() {
            write!(instructions, "\n- {}", CatalogTool::write_guidance()).ok();
        }
        instructions
    }

    /// Whether the catalog tools act on the catalog server of the connected viewer.
    ///
    /// Why: every Rerun viewer hosts its own catalog server, holding the `.rrd` files it opened.
    /// An agent driving a viewer usually asks about that data, and without this default it would
    /// first have to find the server's URL in `viewer_get_state` and pass it to `catalog_connect`.
    ///
    /// This is only a default: `rerun-mcp` also runs without a viewer, and then the agent picks a
    /// catalog server with `catalog_connect`. It is off when a catalog server was configured in
    /// [`crate::CatalogConfig::origin`] (`--catalog-url`), and once the agent connected to one
    /// itself, so an explicit choice always wins.
    pub fn defaults_to_viewer_catalog(&self) -> bool {
        self.defaults_to_viewer_catalog.load(Ordering::Relaxed)
    }

    /// See [`CatalogTools::on_catalog_change`].
    pub fn on_catalog_change(&self, on_change: impl Fn(&re_uri::Origin) + Send + Sync + 'static) {
        self.tools.on_catalog_change(on_change);
    }

    /// The catalog the tools act on, if any.
    pub fn origin(&self) -> Option<re_uri::Origin> {
        self.tools.origin()
    }

    /// Connect to the catalog server at `origin` that a viewer on this machine reported, unless
    /// the agent picked another one.
    ///
    /// Only a catalog server on this machine is used. Why: this connection is made without
    /// asking the user, unlike `catalog_connect`, and it sends the user's stored login. The
    /// callers check that the viewer itself runs on this machine, since every viewer reports its
    /// catalog as a loopback URL, which for a remote viewer would mean a server here.
    pub fn connect_to_viewer_catalog(&self, origin: re_uri::Origin) {
        if origin.is_localhost() {
            self.tools.connect_default(origin);
        } else {
            re_log::debug!("Not connecting to the viewer's remote catalog {origin}");
        }
    }

    /// Run one tool of this toolset.
    pub async fn call(&self, request: CallToolRequestParams) -> CallToolResult {
        let is_connect = CatalogTool::from_name(&request.name) == Some(CatalogTool::Connect);
        let result = toolbox::call(&self.tools, &request.name, request.arguments).await;
        if is_connect && result.is_error != Some(true) {
            self.defaults_to_viewer_catalog
                .store(false, Ordering::Relaxed);
        }
        result
    }
}
