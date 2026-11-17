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
//!
//! The server is exposed two ways — the standalone `rerun-mcp` binary and the
//! `rerun mcp` CLI subcommand.

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

use re_viewer_control_tools::ToolEffect;

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
    /// The viewer toolset, or `None` to leave it out.
    pub viewer: Option<ViewerConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            viewer: Some(ViewerConfig::default()),
        }
    }
}

/// How the viewer toolset starts.
#[derive(Clone, Debug, Default)]
pub struct ViewerConfig {
    /// gRPC address of a running Rerun viewer's `ViewerControlService`,
    /// e.g. `http://127.0.0.1:9876`, the same port the viewer serves SDK connections on.
    ///
    /// When given, the server dials it right away, so the agent can skip `rerun_connect`,
    /// and it becomes the default endpoint for later `rerun_connect` calls.
    /// When `None`, the agent picks the viewer with `rerun_connect`, which defaults to
    /// `http://127.0.0.1:9876`.
    pub endpoint: Option<Url>,
}

/// A recoverable tool failure (not connected, a bad endpoint, a bridge or gRPC error, …), carried
/// as a plain message string.
///
/// It is *not* a JSON-RPC protocol error: a `String` already implements `rmcp`'s `IntoContents`,
/// so when a `#[tool]` method returns `Err(ToolError)`, `rmcp` renders it into a `CallToolResult`
/// with `isError: true` (per the MCP spec). `String` is also the [`egui_mcp::Bridge`]'s error
/// type, so the handlers `?`-propagate bridge failures with no conversion.
type ToolError = String;

/// The result of a tool handler — see [`ToolError`].
type ToolResult<T> = Result<T, ToolError>;

/// The MCP server: the enabled toolsets behind one tool list.
#[derive(Clone)]
struct RerunMcpServer {
    viewer: Option<Arc<ViewerToolset>>,
}

impl RerunMcpServer {
    fn new(config: Config) -> Self {
        let Config { viewer } = config;
        Self {
            viewer: viewer.map(|ViewerConfig { endpoint }| Arc::new(ViewerToolset::new(endpoint))),
        }
    }

    /// Every tool of every enabled toolset.
    fn all_tools(&self) -> Vec<Tool> {
        let Self { viewer } = self;
        viewer.iter().flat_map(|viewer| viewer.tools()).collect()
    }

    /// The MCP `instructions`: one section per enabled toolset.
    fn instructions(&self) -> String {
        let Self { viewer } = self;
        viewer
            .iter()
            .map(|viewer| viewer.instructions())
            .collect::<Vec<_>>()
            .join("\n\n")
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

/// The MCP hints for a tool, telling the agent harness what it can change.
fn annotations(effect: ToolEffect) -> ToolAnnotations {
    match effect {
        ToolEffect::ReadOnly => ToolAnnotations::new()
            .read_only(true)
            .destructive(false)
            .idempotent(true)
            .open_world(false),
        ToolEffect::NonDestructive => ToolAnnotations::new()
            .read_only(false)
            .destructive(false)
            .idempotent(false)
            .open_world(false),
        ToolEffect::Destructive => ToolAnnotations::new()
            .read_only(false)
            .destructive(true)
            .idempotent(false)
            .open_world(true),
    }
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
        let config = ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
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
        let result = match &self.viewer {
            Some(viewer) => viewer.call(request, context).await?,
            None => text_error(format!("unknown tool `{name}`")),
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
pub async fn serve(config: Config) -> anyhow::Result<()> {
    let server = RerunMcpServer::new(config);
    if let Some(viewer) = &server.viewer {
        viewer.connect_on_startup().await;
    }
    let running = server.serve(transport::stdio()).await?;
    let _reason = running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::*;

    #[test]
    fn analytics_names_only_listed_tools() {
        let server = RerunMcpServer::new(Config::default());
        assert_eq!(
            server.tool_name_for_analytics("rerun_connect"),
            "rerun_connect"
        );
        assert_eq!(
            server.tool_name_for_analytics("rerun_get_state"),
            "rerun_get_state"
        );
        assert_eq!(server.tool_name_for_analytics("click"), "click");
        assert_eq!(server.tool_name_for_analytics("connect"), "unknown");
        assert_eq!(
            server.tool_name_for_analytics("/home/me/data.rrd"),
            "unknown"
        );
    }

    #[test]
    fn no_toolsets_means_no_tools() {
        let server = RerunMcpServer::new(Config { viewer: None });
        assert!(server.all_tools().is_empty());
        assert_eq!(server.get_info().instructions, None);
    }

    /// Snapshot of the documentation the llm will see when loading the mcp tools.
    ///
    /// It's useful to look at the snapshot output to check how much llm context the tool
    /// definitions will use.
    #[test]
    fn agent_surface_snapshot() {
        let server = RerunMcpServer::new(Config::default());

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
            write!(surface, "\n## {}\n\n", tool.name).unwrap();
            surface.push_str(&serde_json::to_string_pretty(tool).expect("serialize tool"));
            surface.push('\n');
        }

        insta::assert_snapshot!("agent_surface", surface);
    }

    /// The Anthropic and `OpenAI` APIs reject a tool schema with a top-level `oneOf`, `anyOf`, or
    /// `allOf`, and some agents pass our schemas through to them unchanged.
    #[test]
    fn no_top_level_schema_combinators() {
        for tool in RerunMcpServer::new(Config::default()).all_tools() {
            for keyword in ["allOf", "anyOf", "oneOf"] {
                assert!(
                    !tool.input_schema.contains_key(keyword),
                    "`{}` has a top-level `{keyword}`",
                    tool.name
                );
            }
        }
    }

    #[test]
    fn tool_list_carries_both_cache_hints() {
        let server = RerunMcpServer::new(Config::default());
        let result = serde_json::to_value(tool_list(server.all_tools())).expect("serialize");
        assert!(
            result["ttlMs"].is_u64(),
            "ttlMs missing: {}",
            result["ttlMs"]
        );
        assert_eq!(result["cacheScope"], "public");
    }
}
