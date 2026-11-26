//! `re_rerun_mcp` — an MCP server that lets an agent use Rerun.
//!
//! It sits between two protocols that share nothing but this crate:
//!
//! - **Agent ↔ this server: MCP.** MCP is JSON-RPC over stdio: the agent's harness spawns this
//!   process and exchanges JSON messages on stdin/stdout. The agent sees only JSON — the tool
//!   list with JSON Schemas for their arguments, the instructions prose, and JSON or text
//!   results. It never sees gRPC.
//! - **This server ↔ Rerun: gRPC.** The tools act through Rerun's gRPC services.
//!
//! The tools are grouped into toolsets, each optional (see [`Config`]):
//!
//! - The viewer toolset drives a running viewer through its `ViewerControlService`
//!   (`re_protos`' `viewer_control.proto`), which the viewer serves on the same port as its SDK
//!   connections.
//! - The egui toolset drives the widgets of the viewer the viewer toolset is connected to,
//!   through the tools of `egui_inspection_tools`.
//! - The catalog toolset describes the datasets of a Rerun catalog server through
//!   `re_catalog_tools`, with or without a viewer.
//!
//! The server is exposed two ways — the standalone `rerun-mcp` binary and the
//! `rerun mcp` CLI subcommand.

mod catalog;
mod egui;
mod toolbox;
mod viewer;

use std::sync::Arc;
use std::time::Duration;

use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt as _,
    model::{
        CacheScope, CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock,
        Implementation, ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig,
        Tool, ToolAnnotations,
    },
    service::{RequestContext, RoleServer},
    transport,
};
use url::Url;

pub use re_agent_tools::CatalogAccess;
use re_agent_tools::ToolEffect;
use re_catalog_tools::CatalogTool;

use self::catalog::CatalogToolset;
use self::egui::{EguiToolbox, EguiToolset};
use self::viewer::ViewerToolset;

/// How long a client may treat the tool list as fresh.
///
/// The list cannot change while the process runs, so this only bounds how stale a client's copy
/// may be across a restart that adds or removes an operation.
const TOOL_LIST_TTL: Duration = Duration::from_mins(5);

/// Which toolsets the server offers, and how each one starts.
///
/// The default offers every toolset, none of them bound to an endpoint up front.
#[derive(Clone, Debug)]
pub struct Config {
    /// The viewer toolset, and with it the egui toolset, or `None` to leave both out.
    pub viewer: Option<ViewerConfig>,

    /// The catalog toolset, or `None` to leave it out.
    pub catalog: Option<CatalogConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            viewer: Some(ViewerConfig::default()),
            catalog: Some(CatalogConfig::default()),
        }
    }
}

/// How the viewer toolset starts.
///
/// The default offers the egui toolset too, and starts unconnected.
#[derive(Clone, Debug)]
pub struct ViewerConfig {
    /// gRPC address of a running Rerun viewer's `ViewerControlService`,
    /// e.g. `http://127.0.0.1:9876`, the same port the viewer serves SDK connections on.
    ///
    /// When given, the server dials it right away, so the agent can skip `viewer_connect`,
    /// and it becomes the default endpoint for later `viewer_connect` calls.
    /// When `None`, the agent picks the viewer with `viewer_connect`, which defaults to
    /// `http://127.0.0.1:9876`.
    pub endpoint: Option<Url>,

    /// Whether to also offer the egui toolset, which drives the widgets of the connected viewer
    /// over the viewer toolset's connection.
    pub egui: bool,
}

impl Default for ViewerConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            egui: true,
        }
    }
}

/// How the catalog toolset starts.
#[derive(Clone, Debug, Default)]
pub struct CatalogConfig {
    /// The catalog server the tools act on until the agent calls `catalog_connect`.
    ///
    /// When `None`, the tools act on the catalog of the connected viewer, if it runs on this
    /// machine, until the agent calls `catalog_connect`.
    pub origin: Option<re_uri::Origin>,

    /// Whether to offer the tools that change the catalog.
    pub access: CatalogAccess,
}

/// A recoverable tool failure (not connected, a bad endpoint, a bridge or gRPC error, …), carried
/// as a plain message string.
///
/// It is *not* a JSON-RPC protocol error: a `String` already implements `rmcp`'s `IntoContents`,
/// so when a `#[tool]` method returns `Err(ToolError)`, `rmcp` renders it into a `CallToolResult`
/// with `isError: true` (per the MCP spec).
type ToolError = String;

/// The result of a tool handler — see [`ToolError`].
type ToolResult<T> = Result<T, ToolError>;

/// Why the MCP server could not start, or stopped with an error.
#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error("The MCP server must run inside a Tokio runtime: {source}")]
    NoRuntime { source: re_async::AsyncRuntimeError },

    #[error("Failed to start serving MCP over stdio: {source}")]
    Start {
        source: Box<rmcp::service::ServerInitializeError>,
    },

    #[error("The MCP server stopped unexpectedly: {source}")]
    Stopped { source: tokio::task::JoinError },
}

/// The MCP server: the enabled toolsets behind one tool list.
#[derive(Clone)]
struct RerunMcpServer {
    viewer: Option<Arc<ViewerToolset>>,
    egui: Option<Arc<EguiToolset>>,
    catalog: Option<Arc<CatalogToolset>>,
}

impl RerunMcpServer {
    /// Must be called inside the Tokio runtime the server runs on.
    fn new(config: Config) -> Result<Self, ServeError> {
        let Config { viewer, catalog } = config;
        let (viewer, egui) = match viewer {
            Some(ViewerConfig { endpoint, egui }) => {
                let viewer = Arc::new(ViewerToolset::new(endpoint));
                let egui = egui.then(|| {
                    let viewer = Arc::clone(&viewer);
                    Arc::new(EguiToolset::new(Arc::new(move || viewer.ui_transport())))
                });
                (Some(viewer), egui)
            }
            None => (None, None),
        };
        let catalog = catalog
            .map(|CatalogConfig { origin, access }| {
                CatalogToolset::new(origin, access).map(Arc::new)
            })
            .transpose()?;
        // TODO(RR-4859, RR-4853): remove once the catalog server sends an event for every change.
        if let (Some(viewer), Some(catalog)) = (&viewer, &catalog) {
            let viewer = Arc::clone(viewer);
            catalog.on_catalog_change(move |origin| {
                let viewer = Arc::clone(&viewer);
                let origin = origin.clone();
                tokio::spawn(async move { viewer.show_catalog_changes(&origin).await });
            });
        }
        Ok(Self {
            viewer,
            egui,
            catalog,
        })
    }

    /// Every tool of every enabled toolset.
    fn all_tools(&self) -> Vec<Tool> {
        let Self {
            viewer,
            egui,
            catalog,
        } = self;
        let viewer = viewer.iter().flat_map(|viewer| viewer.tools());
        let egui = egui.iter().flat_map(|egui| egui.tools());
        let catalog = catalog.iter().flat_map(|catalog| catalog.tools());
        itertools::chain!(viewer, egui, catalog).collect()
    }

    /// The MCP `instructions`: one section per enabled toolset.
    fn instructions(&self) -> String {
        let Self {
            viewer,
            egui,
            catalog,
        } = self;
        let catalog = catalog
            .iter()
            .map(|catalog| catalog.instructions(viewer.is_some()));
        let viewer = viewer.iter().map(|viewer| viewer.instructions());
        let egui = egui.iter().map(|_| EguiToolset::instructions().to_owned());
        itertools::chain!(viewer, egui, catalog)
            .map(|section| section.trim().to_owned())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Run a catalog tool, first connecting to the connected viewer's catalog server, if the
    /// catalog tools default to it (see [`CatalogToolset::defaults_to_viewer_catalog`]).
    async fn call_catalog(
        &self,
        catalog: &CatalogToolset,
        request: CallToolRequestParams,
    ) -> CallToolResult {
        if asks_the_viewer_for_its_catalog(&request.name, catalog)
            && let Some(viewer) = &self.viewer
        {
            match viewer.catalog_url().await {
                Ok(Some(origin)) if viewer.is_on_this_machine() => {
                    catalog.connect_to_viewer_catalog(origin);
                }
                Ok(_) => {}
                Err(err) if catalog.origin().is_none() => {
                    return text_error(format!(
                        "No catalog to act on. {err}: call `viewer_connect` to use the viewer's \
                         catalog, or `{}` with a catalog URL.",
                        CatalogTool::Connect.name()
                    ));
                }
                Err(err) => re_log::debug!("Keeping the current catalog: {err}"),
            }
        }
        catalog.call(request).await
    }

    /// `name` must be the tool name from the request.
    fn tool_name_for_analytics(&self, name: &str) -> String {
        if self.all_tools().iter().any(|tool| tool.name == name) {
            name.to_owned()
        } else {
            "unknown".to_owned()
        }
    }
}

/// Whether calling the catalog tool `name` first asks the viewer for its catalog server.
///
/// Only for tools that act on a catalog: the others must work without a viewer.
fn asks_the_viewer_for_its_catalog(name: &str, catalog: &CatalogToolset) -> bool {
    CatalogTool::from_name(name).is_some_and(CatalogTool::acts_on_catalog)
        && catalog.defaults_to_viewer_catalog()
}

/// The MCP hints for a tool, telling the agent harness what it can change.
///
/// `open_world` is [`re_agent_tools::ToolDefinition::open_world`].
fn annotations(effect: ToolEffect, open_world: bool) -> ToolAnnotations {
    ToolAnnotations::new()
        .read_only(effect.read_only())
        .destructive(effect.destructive())
        .idempotent(effect.idempotent())
        .open_world(open_world)
}

/// Shape a recoverable failure as an `isError: true` tool result, for the `ServerHandler`
/// methods that return a [`CallToolResponse`] rather than a [`ToolResult`].
fn text_error(msg: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(msg.into())])
}

/// Collapse a router response to its completed result.
///
/// None of these tools use multi-round-trip input or tasks, so any other variant is a bug.
fn complete(response: CallToolResponse) -> CallToolResult {
    match response {
        CallToolResponse::Complete(result) => result,
        other => text_error(format!("unexpected tool response: {other:?}")),
    }
}

/// The `tools/list` result, with the cache hints MCP 2026-07-28 (SEP-2549) requires.
///
/// Both `ttlMs` and `cacheScope` are mandatory from that protocol version on: a client that
/// negotiated it rejects the whole list when either is missing, leaving the server with no tools.
/// The list is fixed for the life of the process and holds nothing user-specific, so it may be
/// cached and shared freely.
fn tool_list(tools: Vec<Tool>) -> ListToolsResult {
    ListToolsResult::with_all_items(tools)
        .with_ttl_ms(TOOL_LIST_TTL.as_millis() as u64)
        .with_cache_scope(CacheScope::Public)
}

impl ServerHandler for RerunMcpServer {
    fn get_info(&self) -> ServerConfig {
        let config = ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_tool_list_changed()
                .build(),
        )
        .with_server_info(Implementation::new("rerun-mcp", env!("CARGO_PKG_VERSION")));
        let instructions = self.instructions();
        if instructions.is_empty() {
            config
        } else {
            config.with_instructions(instructions)
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(tool_list(self.all_tools()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let name = request.name.clone();
        let result = match (&self.viewer, &self.egui, &self.catalog) {
            (_, _, Some(catalog)) if CatalogToolset::owns(&name) => {
                self.call_catalog(catalog, request).await
            }
            (Some(viewer), Some(egui), _) if EguiToolbox::owns(&name) => {
                let mut result = egui.call(&name, request.arguments).await;
                viewer.append_new_logs(&mut result).await;
                result
            }
            (Some(viewer), _, _) => viewer.call(request, context).await?,
            (None, _, _) => text_error(format!("unknown tool `{name}`")),
        };

        let is_error = result.is_error == Some(true);
        re_analytics::record(|| re_analytics::event::McpToolCalled {
            tool: self.tool_name_for_analytics(&name),
            is_error,
        });
        Ok(result.into())
    }
}

/// Serve the MCP server over stdio until the MCP client (the agent) disconnects.
///
/// Must run inside a Tokio runtime, and assumes logging is already set up.
/// Both the `rerun mcp` subcommand and the standalone `rerun-mcp` binary call this.
pub async fn serve(config: Config) -> Result<(), ServeError> {
    // The catalog tools may reach `https` servers.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let server = RerunMcpServer::new(config)?;
    if let Some(viewer) = &server.viewer {
        viewer.connect_on_startup().await;
    }
    let running = server
        .serve(transport::stdio())
        .await
        .map_err(|source| ServeError::Start {
            source: Box::new(source),
        })?;
    let _reason = running
        .waiting()
        .await
        .map_err(|source| ServeError::Stopped { source })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    fn server(viewer: Option<ViewerConfig>, catalog: Option<CatalogConfig>) -> RerunMcpServer {
        RerunMcpServer::new(Config { viewer, catalog }).expect("server")
    }

    /// Every `backticked` word of `text` that could be a tool name.
    pub fn backticked_names(text: &str) -> impl Iterator<Item = &str> {
        text.split('`')
            .skip(1)
            .step_by(2)
            .filter(|name| name.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
    }

    fn default_server() -> RerunMcpServer {
        RerunMcpServer::new(Config::default()).expect("server")
    }

    #[tokio::test]
    async fn analytics_names_only_listed_tools() {
        let server = default_server();
        for name in [
            "viewer_connect",
            "viewer_get_state",
            "ui_click",
            "catalog_list_entries",
        ] {
            assert_eq!(server.tool_name_for_analytics(name), name);
        }
        assert_eq!(server.tool_name_for_analytics("connect"), "unknown");
        assert_eq!(
            server.tool_name_for_analytics("/home/me/data.rrd"),
            "unknown"
        );
    }

    #[tokio::test]
    async fn no_toolsets_means_no_tools() {
        let server = server(None, None);
        assert!(server.all_tools().is_empty());
        assert_eq!(server.get_info().instructions, None);
    }

    #[tokio::test]
    async fn the_egui_toolset_is_optional() {
        let has_egui_tools = |server: &RerunMcpServer| {
            server
                .all_tools()
                .iter()
                .any(|tool| tool.name == "ui_click")
        };
        assert!(has_egui_tools(&default_server()));

        let without_egui = server(
            Some(ViewerConfig {
                endpoint: None,
                egui: false,
            }),
            None,
        );
        assert!(!has_egui_tools(&without_egui));
        let instructions = without_egui.instructions();
        assert!(!instructions.contains("widget_tree"), "{instructions}");
    }

    #[tokio::test]
    async fn egui_instructions_name_available_tools() {
        /// The argument and field names the instructions mention.
        const NOT_TOOLS: &[&str] = &[
            "bounds",
            "content_contains",
            "id",
            "label",
            "pos",
            "rect",
            "role",
        ];

        let tools = default_server().all_tools();
        let names =
            backticked_names(EguiToolset::instructions()).filter(|name| !NOT_TOOLS.contains(name));
        for name in names {
            assert!(
                tools.iter().any(|tool| tool.name == name),
                "unknown tool: {name}"
            );
        }
    }

    #[tokio::test]
    async fn a_catalog_only_server_has_no_viewer_tools() {
        let server = server(None, Some(CatalogConfig::default()));
        let names: Vec<String> = server
            .all_tools()
            .into_iter()
            .map(|tool| tool.name.into_owned())
            .collect();
        assert_eq!(
            names,
            CatalogTool::available()
                .map(|tool| tool.name().to_owned())
                .collect::<Vec<_>>()
        );

        let result = server
            .call_catalog(
                server.catalog.as_ref().expect("catalog toolset"),
                CallToolRequestParams::new("catalog_list_entries"),
            )
            .await;
        assert_eq!(result.is_error, Some(true));
    }

    #[tokio::test]
    async fn without_a_viewer_or_catalog_the_error_names_both_ways_out() {
        let server = default_server();
        let result = server
            .call_catalog(
                server.catalog.as_ref().expect("catalog toolset"),
                CallToolRequestParams::new("catalog_list_entries"),
            )
            .await;
        assert_eq!(result.is_error, Some(true));
        let text = serde_json::to_string(&result.content).expect("serialize");
        assert!(text.contains("viewer_connect"), "{text}");
        assert!(text.contains("catalog_connect"), "{text}");
    }

    #[tokio::test]
    async fn a_configured_catalog_is_not_replaced_by_the_viewers() {
        let server = RerunMcpServer::new(Config {
            viewer: Some(ViewerConfig::default()),
            catalog: Some(CatalogConfig {
                origin: Some("rerun+http://127.0.0.1:51234".parse().expect("valid")),
                access: CatalogAccess::ReadWrite,
            }),
        })
        .expect("server");
        let catalog = server.catalog.as_ref().expect("catalog toolset");
        assert!(!catalog.defaults_to_viewer_catalog());
        let instructions = server.instructions();
        assert!(
            instructions.contains("acting on the catalog at `rerun+http://127.0.0.1:51234`"),
            "{instructions}"
        );
    }

    #[tokio::test]
    async fn a_read_only_catalog_offers_no_writes() {
        let server = server(
            None,
            Some(CatalogConfig {
                origin: None,
                access: CatalogAccess::ReadOnly,
            }),
        );
        let names: Vec<String> = server
            .all_tools()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect();
        assert!(names.iter().any(|name| name == "catalog_list_entries"));
        assert!(!names.iter().any(|name| name == "catalog_create_datasets"));
        let instructions = server.instructions();
        assert!(
            !instructions.contains("catalog_create_datasets"),
            "{instructions}"
        );
    }

    #[tokio::test]
    async fn only_a_local_viewer_catalog_is_used() {
        let server = default_server();
        let catalog = server.catalog.as_ref().expect("catalog toolset");
        catalog.connect_to_viewer_catalog("rerun+http://example.com:51234".parse().expect("valid"));
        assert_eq!(catalog.origin(), None);
        catalog.connect_to_viewer_catalog("rerun+http://127.0.0.1:51234".parse().expect("valid"));
        assert_eq!(
            catalog.origin().map(|origin| origin.to_string()),
            Some("rerun+http://127.0.0.1:51234".to_owned())
        );
    }

    #[tokio::test]
    async fn only_default_catalog_calls_ask_the_viewer() {
        let default = default_server();
        let catalog = default.catalog.as_ref().expect("catalog toolset");
        assert!(asks_the_viewer_for_its_catalog(
            "catalog_list_entries",
            catalog
        ));
        assert!(!asks_the_viewer_for_its_catalog("catalog_connect", catalog));

        let configured = server(
            Some(ViewerConfig::default()),
            Some(CatalogConfig {
                origin: Some("rerun+http://127.0.0.1:51234".parse().expect("valid")),
                access: CatalogAccess::ReadWrite,
            }),
        );
        let catalog = configured.catalog.as_ref().expect("catalog toolset");
        assert!(!asks_the_viewer_for_its_catalog(
            "catalog_list_entries",
            catalog
        ));
    }

    /// `catalog_connect` picks a server itself, so it fails on that server, not on the missing
    /// viewer.
    #[tokio::test(flavor = "multi_thread")]
    async fn catalog_connect_does_not_need_a_viewer() {
        let server = default_server();
        let mut request = CallToolRequestParams::new("catalog_connect");
        request.arguments = Some(
            serde_json::json!({ "url": "rerun+http://127.0.0.1:2" })
                .as_object()
                .expect("object")
                .clone(),
        );
        let result = server
            .call_catalog(server.catalog.as_ref().expect("catalog toolset"), request)
            .await;
        let text = serde_json::to_string(&result.content).expect("serialize");
        assert_eq!(result.is_error, Some(true));
        assert!(text.contains("connect to the catalog"), "{text}");
        assert!(!text.contains("viewer_connect"), "{text}");
    }

    #[tokio::test]
    async fn only_tools_that_act_on_a_catalog_ask_the_viewer() {
        let server = default_server();
        let catalog = server.catalog.as_ref().expect("catalog toolset");
        let asking: Vec<_> = CatalogTool::available()
            .filter(|tool| asks_the_viewer_for_its_catalog(tool.name(), catalog))
            .collect();
        let expected: Vec<_> = CatalogTool::available()
            .filter(|tool| {
                !matches!(
                    tool,
                    CatalogTool::ListServers | CatalogTool::Connect | CatalogTool::Login
                )
            })
            .collect();
        assert_eq!(asking, expected);
    }

    #[tokio::test]
    async fn instructions_match_the_toolsets() {
        let both = default_server().instructions();
        assert!(both.contains("catalog of the connected viewer"), "{both}");
        assert!(!both.contains("\n\n\n"), "{both}");

        let catalog_only = server(None, Some(CatalogConfig::default())).instructions();
        assert!(!catalog_only.contains("viewer_"), "{catalog_only}");
        assert!(
            catalog_only.contains("Call `catalog_connect` with the catalog's URL first."),
            "{catalog_only}"
        );

        let viewer_only = server(Some(ViewerConfig::default()), None).instructions();
        for tool in CatalogTool::available() {
            assert!(!viewer_only.contains(tool.name()), "{viewer_only}");
        }
    }

    /// Snapshot of the documentation the llm will see when loading the mcp tools.
    ///
    /// It's useful to look at the snapshot output to check how much llm context the tool
    /// definitions will use.
    #[tokio::test]
    async fn agent_surface_snapshot() {
        let server = default_server();

        let mut surface = String::new();
        surface.push_str("# Server instructions\n\n");
        surface.push_str(
            server
                .get_info()
                .instructions
                .as_deref()
                .unwrap_or("(none)"),
        );
        surface.push_str("\n\n# Tools\n");

        // Exactly what `list_tools` serves, so the snapshot cannot drift from the real surface.
        let mut tools = server.all_tools();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        for tool in &tools {
            write!(surface, "\n## {}\n\n", tool.name).expect("write to a string");
            surface.push_str(&serde_json::to_string_pretty(tool).expect("serialize tool"));
            surface.push('\n');
        }

        insta::assert_snapshot!("agent_surface", surface);
    }

    /// The Anthropic and `OpenAI` APIs reject a tool schema with a top-level `oneOf`, `anyOf`, or
    /// `allOf`, and some agents pass our schemas through to them unchanged.
    #[tokio::test]
    async fn no_top_level_schema_combinators() {
        for tool in default_server().all_tools() {
            for keyword in ["allOf", "anyOf", "oneOf"] {
                assert!(
                    !tool.input_schema.contains_key(keyword),
                    "`{}` has a top-level `{keyword}`",
                    tool.name
                );
            }
        }
    }

    #[tokio::test]
    async fn tool_list_carries_both_cache_hints() {
        let server = default_server();
        let result = serde_json::to_value(tool_list(server.all_tools())).expect("serialize");
        assert!(
            result["ttlMs"].is_u64(),
            "ttlMs missing: {}",
            result["ttlMs"]
        );
        assert_eq!(result["cacheScope"], "public");
    }
}
