//! `re_viewer_mcp` — an MCP server that lets an agent drive the Rerun viewer.
//!
//! It sits between two protocols that share nothing but this crate:
//!
//! - **Agent ↔ this server: MCP.** MCP is JSON-RPC over stdio: the agent's harness spawns this
//!   process and exchanges JSON messages on stdin/stdout. The agent sees only JSON — the tool
//!   list with JSON Schemas for their arguments, the `INSTRUCTIONS` prose, and JSON or text
//!   results. It never sees gRPC, but it does read the `.proto` documentation: `proto_tools`
//!   derives each viewer-control tool's schema and description from the descriptor set, which
//!   carries the comments.
//! - **This server ↔ the viewer: gRPC.** Every tool call becomes one call on the viewer's
//!   `ViewerControlService` (`re_protos`' `viewer_control.proto`), which the viewer serves on the same port
//!   as its SDK connections.
//!
//! The tools come in two groups.
//!
//! The egui UI tools (`query_tree`, `screenshot`, `click`, …) are `egui_mcp`'s, reused unchanged.
//! Each call becomes one `egui_inspection` request/response exchange carried inside a single
//! `egui_inspect` operation instead of `egui_mcp`'s local inspection socket.
//!
//! The viewer-control tools (`rerun_get_viewer_state`, `rerun_set_time_cursor`, `rerun_close_recordings`, …) are
//! generated from `viewer_control.proto`: one tool per operation in the request envelope's
//! `oneof`. `rerun_connect` and `rerun_disconnect` are hand-written, since they manage the connection itself
//! rather than driving the viewer.
//!
//! The server is exposed two ways — the standalone `re-viewer-mcp` binary and the
//! `rerun viewer-mcp` CLI subcommand.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt as _,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::{
        CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
    },
    schemars,
    service::{RequestContext, RoleServer},
    tool, tool_router, transport,
};
mod proto_tools;

use serde::Deserialize;
use tonic::transport::Channel;
use url::Url;

use egui_inspection::protocol::{self, PROTOCOL_VERSION, Request, Response};
use egui_mcp::{BoxFuture, Bridge, PeerInfo, Transport, UiServer};
use re_protos::viewer_control::v1alpha1::{
    EguiInspectRequest, GetViewerLogsRequest, ViewerControlOp, ViewerLogEntry,
    viewer_control_service_client::ViewerControlServiceClient,
};

const DEFAULT_VIEWER_ENDPOINT: &str = "http://127.0.0.1:9876";

/// Deadline for one viewer-control operation.
///
/// These are answered from viewer state without waiting for a frame, so anything this slow means
/// the viewer is gone rather than busy.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Deadline for one `egui_inspection` exchange.
///
/// A UI request should be as quick as the interaction it stands for, so this allows only for
/// network hiccups and for the slow transfer of a screenshot. A viewer that takes longer is one
/// to fix rather than to wait for.
const UI_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Send one viewer-control operation over `ViewerControlService::ViewerControl` and unwrap the
/// matching response, so callers name a request type rather than the `oneof` variants.
async fn execute<Op: ViewerControlOp>(
    client: &mut ViewerControlServiceClient<Channel>,
    request: Op,
) -> Result<Op::Response, String> {
    let response = tokio::time::timeout(
        REQUEST_TIMEOUT,
        client.viewer_control(request.into_envelope()),
    )
    .await
    .map_err(|_elapsed| format!("The viewer did not answer within {REQUEST_TIMEOUT:?}"))?
    .map_err(|err| err.to_string())?
    .into_inner();
    Op::from_envelope(response).map_err(|err| err.to_string())
}

/// An [`egui_mcp::Transport`] that carries each `egui_inspection` request/response over a unary
/// `EguiInspect` gRPC call to the running viewer.
#[derive(Clone)]
struct GrpcInspector {
    client: ViewerControlServiceClient<Channel>,
}

impl Transport for GrpcInspector {
    fn request(&self, req: Request) -> BoxFuture<'_, Result<Response, String>> {
        Box::pin(async move {
            let request = protocol::encode_body(&req).map_err(|err| err.to_string())?;
            let mut client = self.client.clone();
            let response = tokio::time::timeout(
                UI_REQUEST_TIMEOUT,
                client.egui_inspect(EguiInspectRequest { request }),
            )
            .await
            .map_err(|_elapsed| {
                format!(
                    "The viewer did not paint a frame within {UI_REQUEST_TIMEOUT:?}. \
                     Every UI tool waits on the frame loop, so a long import or an unresponsive \
                     window stalls all of them. `rerun_get_viewer_state` and \
                     `rerun_get_viewer_logs` are answered without a frame and still work: use \
                     them to see whether the viewer is busy loading, and retry once it is idle."
                )
            })?
            .map_err(|err| format!("egui_inspect rpc failed: {err}"))?
            .into_inner();
            protocol::decode_body(&response.response).map_err(|err| err.to_string())
        })
    }
}

/// Dial the viewer once and build both the [`Bridge`] (which tunnels the egui tools over the
/// unary `egui_inspect` operation) and the gRPC client the rerun-specific tools call through —
/// sharing the single connection between them.
async fn connect_grpc(
    endpoint: &Url,
) -> Result<(Bridge, ViewerControlServiceClient<Channel>), String> {
    // No `Endpoint::timeout` here: that is one deadline for every RPC on the channel, and the
    // two kinds of call have very different expectations of how long the viewer may take.
    // Each call site applies its own instead.
    let channel = tonic::transport::Endpoint::from_shared(endpoint.to_string())
        .map_err(|err| err.to_string())?
        .connect_timeout(REQUEST_TIMEOUT)
        .connect()
        .await
        .map_err(|err| err.to_string())?;
    let client = ViewerControlServiceClient::new(channel);
    let inspector = GrpcInspector {
        client: client.clone(),
    };

    // Read the peer's label up front (also a liveness check), matching the TCP `attach` path.
    let label = match inspector.request(Request::GetInfo).await? {
        Response::Info { label, .. } => label,
        Response::Error { message } => return Err(message),
        _ => return Err("unexpected response to GetInfo".to_owned()),
    };

    let bridge = Bridge::with_transport(
        inspector,
        PeerInfo {
            transport: endpoint.to_string(),
            protocol_version: PROTOCOL_VERSION,
            label,
        },
    );
    Ok((bridge, client))
}

/// The live connection to the viewer: the egui [`UiServer`] (which tunnels the egui tools over
/// the `egui_inspect` operation) and the raw gRPC client (used by the rerun-specific tools).
/// Both are established together on `rerun_connect` and dropped together on `rerun_disconnect`, so they live
/// behind a single lock.
struct Connection {
    ui: UiServer,
    client: ViewerControlServiceClient<Channel>,

    /// The Viewer gRPC endpoint this connection was dialed with.
    viewer_endpoint: Url,
    peer: PeerInfo,

    /// Sequence number of the last viewer log entry appended to a tool result.
    /// Starts at the newest entry at connect time, so old logs are not replayed.
    log_cursor: AtomicU64,
}

/// Prefix on every tool this crate defines, so a `rerun_*` name is a viewer-control operation and
/// an unprefixed one is an egui widget tool.
///
/// The egui tools keep their upstream names on purpose: they are the same tools the standalone
/// `egui-mcp` binary serves, and its `batch` tool re-enters its own router by unprefixed name, so
/// renaming them here would make a name inside `batch` differ from the same name outside it.
const RERUN_PREFIX: &str = "rerun_";

/// Tools whose results cannot include viewer log entries.
const TOOLS_WITHOUT_LOG: &[&str] = &["rerun_disconnect"];

/// The `re_viewer_mcp` server: rerun-specific connection / state tools, plus the reusable
/// `egui_mcp` [`UiServer`], built on `rerun_connect` and dropped on `rerun_disconnect`, that drives the live
/// viewer.
#[derive(Clone)]
struct ViewerMcpServer {
    /// The active connection, `Some` while connected and `None` otherwise. Tool handlers clone
    /// the `Arc<Connection>` out of the lock (sync, so it can't be held across an await) and
    /// then use it freely.
    conn: Arc<Mutex<Option<Arc<Connection>>>>,

    /// Router over the egui UI/inspection tools. Independent of the connection, so the tools
    /// stay listed while disconnected; a call before `rerun_connect` returns `no app connected`.
    ui_router: ToolRouter<UiServer>,

    /// Router for the rerun-specific tools layered on top of the egui ones.
    tool_router: ToolRouter<Self>,

    /// Set when the server was started for one specific viewer, which it connects to on startup.
    viewer_endpoint: Option<Url>,
}

/// Arguments for the `rerun_connect` tool.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct ConnectArgs {
    /// gRPC endpoint of the running viewer's `ViewerControlService`.
    /// Defaults to the endpoint the server was started for, else `http://127.0.0.1:9876`.
    #[serde(default)]
    #[schemars(with = "Option<String>")]
    endpoint: Option<Url>,
}

/// Arguments for the tools that take none.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct EmptyArgs {}

#[tool_router]
impl ViewerMcpServer {
    fn new(viewer_endpoint: Option<Url>) -> Self {
        Self {
            conn: Arc::new(Mutex::new(None)),
            ui_router: UiServer::router(),
            tool_router: Self::tool_router(),
            viewer_endpoint,
        }
    }

    /// Dial `endpoint` and install the connection.
    ///
    /// Connecting again to the endpoint already connected is a no-op. Connecting to a different
    /// endpoint replaces the old connection after the new one succeeds.
    async fn connect_to(&self, endpoint: &Url) -> ToolResult<PeerInfo> {
        if let Some(conn) = self.conn.lock().as_ref()
            && conn.viewer_endpoint == *endpoint
        {
            return Ok(conn.peer.clone());
        }
        let (bridge, mut client) = connect_grpc(endpoint)
            .await
            .map_err(|err| format!("connect failed: {err}"))?;
        let peer = bridge.peer_info.clone();
        let newest_log = execute(
            &mut client,
            GetViewerLogsRequest {
                after_sequence: None,
            },
        )
        .await
        .ok()
        .and_then(|response| response.entries.last().map(|e| e.sequence))
        .unwrap_or(0);
        *self.conn.lock() = Some(Arc::new(Connection {
            ui: egui_mcp::UiServer::new(bridge),
            client,
            viewer_endpoint: endpoint.clone(),
            peer: peer.clone(),
            log_cursor: AtomicU64::new(newest_log),
        }));
        Ok(peer)
    }

    /// Appends the viewer log entries since the previous tool call to `result`,
    /// so the agent sees warnings and errors as they happen.
    async fn append_new_logs(&self, result: &mut CallToolResult) {
        let conn = self.conn.lock().clone();
        let Some(conn) = conn else {
            return;
        };
        let after_sequence = conn.log_cursor.load(Ordering::Relaxed);
        let mut client = conn.client.clone();
        let Ok(response) = execute(
            &mut client,
            GetViewerLogsRequest {
                after_sequence: Some(after_sequence),
            },
        )
        .await
        else {
            return;
        };
        let entries = response.entries;
        let Some(last) = entries.last() else {
            return;
        };
        conn.log_cursor.store(last.sequence, Ordering::Relaxed);
        result.content.push(Content::text(format!(
            "Viewer log since the previous tool call:\n{}",
            format_log_entries(&entries)
        )));
    }

    /// Every tool this server offers: the connection tools, the operations generated from
    /// `viewer_control.proto`, and the reusable egui UI tools.
    ///
    /// Both routers are independent of the connection, so their tools stay listed even while
    /// disconnected.
    fn all_tools(&self) -> Vec<Tool> {
        let mut tools = self.tool_router.list_all();
        tools.extend(proto_tools::tools());
        for tool in &mut tools {
            tool.name = std::borrow::Cow::Owned(format!("{RERUN_PREFIX}{}", tool.name));
        }

        // The egui tools keep their upstream names, so they are added after the prefixing.
        tools.extend(self.ui_router.list_all());
        tools
    }

    /// Run one operation generated from `viewer_control.proto`.
    ///
    /// The arguments are validated against the operation's descriptor, sent as the single
    /// `ViewerControl` RPC, and the response rendered back as canonical protobuf JSON, so nothing
    /// here names a specific operation.
    async fn call_operation(
        &self,
        name: &str,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> ToolResult<CallToolResult> {
        let request = proto_tools::build_request(name, arguments)?;
        let mut client = self.client()?;
        let response = client
            .viewer_control(request)
            .await
            .map_err(|err| format!("{name} failed: {err}"))?
            .into_inner();
        let json = proto_tools::response_json(name, &response)
            .map_err(|err| format!("{name} failed: {err}"))?;
        Ok(CallToolResult::success(vec![Content::text(
            json.to_string(),
        )]))
    }

    /// The MCP `instructions`: [`INSTRUCTIONS`], led by the configured viewer's connection state.
    fn instructions(&self) -> String {
        match &self.viewer_endpoint {
            Some(endpoint) if self.conn.lock().is_some() => format!(
                "This server was started for the Rerun viewer at `{endpoint}` and is already connected to it. \
                 Do not call `rerun_connect`; start using the other tools right away.\n\n{INSTRUCTIONS}"
            ),
            Some(endpoint) => format!(
                "This server was started for the Rerun viewer at `{endpoint}`, but it is not connected yet. \
                 Call `rerun_connect` before using the other tools.\n\n{INSTRUCTIONS}"
            ),
            None => INSTRUCTIONS.to_owned(),
        }
    }

    /// The connected viewer's gRPC client, for the rerun-specific tools. Returns an owned clone
    /// (tonic clients are cheap to clone and share the channel) so the RPC runs without holding
    /// the lock, and errors with `not connected` when nothing is connected.
    fn client(&self) -> ToolResult<ViewerControlServiceClient<Channel>> {
        self.conn
            .lock()
            .as_ref()
            .map(|c| c.client.clone())
            .ok_or_else(|| "not connected — call `rerun_connect` first".to_owned())
    }

    /// Connect to a running Rerun viewer over gRPC.
    ///
    /// The other tools will be available once the connection is established.
    /// `endpoint` defaults to the viewer this server was started for, else `http://127.0.0.1:9876`
    /// (the viewer's default gRPC address).
    /// Connecting again to the same endpoint is a no-op. Connecting to a different endpoint
    /// replaces the old connection once the new connection succeeds. Call `rerun_disconnect` to drop
    /// the connection without replacing it.
    #[tool]
    async fn connect(
        &self,
        Parameters(args): Parameters<ConnectArgs>,
    ) -> ToolResult<CallToolResult> {
        let endpoint = args
            .endpoint
            .or_else(|| self.viewer_endpoint.clone())
            .map_or_else(
                || Url::parse(DEFAULT_VIEWER_ENDPOINT).map_err(|err| err.to_string()),
                Ok,
            )?;
        let peer = self.connect_to(&endpoint).await?;
        Ok(CallToolResult::structured(serde_json::json!({
            "ok": true,
            "connected": endpoint,
            "peer": peer,
        })))
    }

    /// Disconnect from the viewer, dropping the gRPC-backed bridge.
    /// The tools stop working until `rerun_connect` is called again.
    #[tool]
    async fn disconnect(
        &self,
        Parameters(_args): Parameters<EmptyArgs>,
    ) -> ToolResult<CallToolResult> {
        if self.conn.lock().take().is_some() {
            Ok(CallToolResult::structured(
                serde_json::json!({ "ok": true }),
            ))
        } else {
            Err("not connected".to_owned())
        }
    }
}

/// One line per entry: `#sequence [LEVEL target] message`.
fn format_log_entries(entries: &[ViewerLogEntry]) -> String {
    entries
        .iter()
        .map(|entry| {
            let ViewerLogEntry {
                sequence,
                level,
                target,
                message,
            } = entry;
            format!("#{sequence} [{level} {target}] {message}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A recoverable tool failure (not connected, a bad endpoint, a bridge or gRPC error, …), carried
/// as a plain message string.
///
/// It is *not* a JSON-RPC protocol error: a `String` already implements `rmcp`'s `IntoContents`,
/// so when a `#[tool]` method returns `Err(ToolError)`, `rmcp` renders it into a `CallToolResult`
/// with `isError: true` (per the MCP spec). `String` is also the [`Bridge`]'s error type, so the
/// handlers `?`-propagate bridge failures with no conversion.
type ToolError = String;

/// The result of a tool handler — see [`ToolError`].
type ToolResult<T> = Result<T, ToolError>;

/// Shape a recoverable failure as an `isError: true` tool result, for the `ServerHandler`
/// methods that return `Result<CallToolResult, McpError>` rather than a [`ToolResult`].
fn text_error(msg: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![Content::text(msg.into())])
}

/// Operating guidance sent to clients at initialize (the MCP `instructions` field). The per-tool
/// descriptions cover each command in isolation; this establishes the cross-cutting workflow —
/// `rerun_connect` first, then the observe→act→verify loop the egui tools share — that an agent
/// otherwise has to infer.
const INSTRUCTIONS: &str = r#"This MCP drives a live Rerun viewer: it reads the viewer's accessibility tree and synthesizes real input events. Work in an observe → act → verify loop.

Getting oriented:
- Call `rerun_connect` first (it dials the viewer's gRPC server); every other tool errors until then. A server started for a specific viewer is already connected and says so above.
- If no viewer is running, launch one. If the user tells you to work in the background, or no desktop is available, use `--headless`.
- Every Rerun gRPC endpoint serves gRPC server reflection, so `grpcurl -plaintext <host:port> list` shows which services an address speaks (viewer control, SDK proxy, catalog) before you `rerun_connect`, and `describe` shows a service's methods and message types.
- The tool name tells you which of two families it belongs to. A `rerun_*` tool is high level: it names a viewer action and carries it out in one call. Every other tool (`query_tree`, `click`, `type_text`, `hover`, `scroll`, …) is low level: it drives the widgets one input event at a time, the way a person would.
- Prefer a `rerun_*` tool whenever one fits, and drop to the low-level tools only for what they do not cover. Clicking through the UI costs several calls and a lot of context to achieve what one `rerun_*` call does, and it breaks whenever the layout moves.
- Start most tasks with `rerun_get_viewer_state` to see what is loaded, then `query_tree` to find widgets, and/or `screenshot` to see the rendered frame.

Targeting widgets:
- Prefer locators — an `id` from `query_tree`, or `role`/`label_contains` — over a raw `pos`. Locators resolve to the widget's current position and survive layout changes; reach for `pos` only when nothing matches.

Acting and verifying:
- After an action that changes the UI, confirm it landed: `query_tree` for the expected state, `screenshot` to look, or `wait_for` to poll until async or animated UI settles.
- Confirm a load with `rerun_get_viewer_state`, not `screenshot`. It names the recordings, timelines and views that appeared, which is what "did it load?" actually asks; a full-window screenshot costs far more and answers less. Screenshot when the question is about looks — framing, layout, colors.
- Use `batch` to act and observe in one round trip (e.g. `click` then `screenshot`), avoiding an extra turn.
- To move through time, call `rerun_get_viewer_state` for the recordings/timelines and their valid ranges, then `rerun_set_time_cursor`.
- The viewer's log messages (INFO and above) since the previous tool call are appended to every tool result. Read them: a warning or error there usually explains what the user is seeing. `rerun_get_viewer_logs` fetches older messages.
- `rerun_close_recordings` clears recordings away. Iterating on a file you keep regenerating leaves a pile of stale recordings behind, which makes `rerun_get_viewer_state` and the UI hard to read — close them.

Reading the data itself:
- Never guess an entity path or a component name. `rerun_get_recording_schema` names every entity of an open recording, the components logged on each, and their Arrow datatypes, whatever the recording was loaded from. Read it before you write a query, a blueprint, or a sentence describing the data.
- That is the schema, not the values: it says what was logged at some point, not what is there at the current time. These tools drive the UI and do not read values — the viewer hosts a catalog server, so read the real values through the Python API.
- `rerun_get_viewer_state` reports that server as `catalog_url`. Hand it straight to `CatalogClient`; do not hardcode a port, since a viewer may serve on any of them.
- A recording's `store_id` is the string `{kind}:{application_id}:{recording_id}`. The kind runs to the first colon, the application id to the next colon not preceded by a backslash, and the recording id is the rest; a colon inside the application id is escaped as `\:` (and a backslash as `\\`). The application id is the catalog dataset's **id** (not its name) and the recording id is the segment id, so look the dataset up by id:
  `CatalogClient(catalog_url).get_dataset(id=application_id)`, then `.schema().entity_paths()` for the schema and `.segment_store(recording_id)` for the data.
- Only local `.rrd` and `.rbl` files are registered, so a recording streamed from an SDK, opened from an `http(s)` URL, or imported from a directory or another file format is absent from the catalog, and its application id is the plain application id rather than a dataset id. Read those from the source instead (`rerun.chunk.RrdReader(path).store(...).schema()` and friends), fetching a URL to a temporary file first.
- `rerun_close_recordings` only closes recordings in the viewer; registered recordings stay in the catalog and can still be read and reopened afterwards.

Conventions:
- Everything is in logical points, one shared coordinate frame: raw `pos`, `resize` dimensions, the `bounds` from `query_tree`/`get_node`, and a default (`pixels_per_point: 1.0`) `screenshot`. So a node's `bounds` center is exactly where to `click`, and a pixel in the screenshot is a logical point. There is no fixed screen size; use `resize` to set the viewport."#;

impl ServerHandler for ViewerMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("viewer-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(self.instructions())
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: self.all_tools(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let name = request.name.clone();

        // Our own tools carry the prefix. The egui ones keep their upstream names and go to the
        // attached UI server, which exists only while connected.
        let ours = name
            .strip_prefix(RERUN_PREFIX)
            .map(ToOwned::to_owned)
            .filter(|name| self.tool_router.has_route(name) || proto_tools::is_operation(name));

        let mut result = if let Some(ours) = ours {
            if self.tool_router.has_route(&ours) {
                let mut request = request;
                request.name = std::borrow::Cow::Owned(ours);
                self.tool_router
                    .call(ToolCallContext::new(self, request, context))
                    .await?
            } else {
                match self.call_operation(&ours, request.arguments).await {
                    Ok(result) => result,
                    Err(err) => text_error(err),
                }
            }
        } else {
            let conn = self.conn.lock().clone();
            let Some(conn) = conn else {
                return Ok(text_error("no app connected — call `rerun_connect` first"));
            };
            conn.ui.dispatch(&self.ui_router, request, context).await?
        };

        if !TOOLS_WITHOUT_LOG.contains(&name.as_ref()) {
            self.append_new_logs(&mut result).await;
        }
        Ok(result)
    }
}

/// Serve the MCP server over stdio until the MCP client (the agent) disconnects.
///
/// `viewer_endpoint` is the gRPC address of a running Rerun viewer's `ViewerControlService`,
/// e.g. `http://127.0.0.1:9876`, the same port the viewer serves SDK connections on.
/// When given, the server dials it right away, so the agent can skip `rerun_connect`,
/// and it becomes the default endpoint for later `rerun_connect` calls.
/// When `None`, the agent picks the viewer with `rerun_connect`, which defaults to
/// `http://127.0.0.1:9876`.
///
/// A failed eager connect is only logged: the viewer may come up later, and `rerun_connect` still works.
///
/// Must run inside a Tokio runtime, and assumes logging is already set up.
/// Both the `rerun viewer-mcp` subcommand and the standalone `re-viewer-mcp` binary call this.
pub async fn serve(viewer_endpoint: Option<Url>) -> anyhow::Result<()> {
    let server = ViewerMcpServer::new(viewer_endpoint.clone());
    if let Some(endpoint) = viewer_endpoint {
        match server.connect_to(&endpoint).await {
            Ok(_) => re_log::info!("Connected to viewer at {endpoint}"),
            Err(err) => re_log::warn!("Failed to connect to viewer at {endpoint}: {err}"),
        }
    }
    let running = server.serve(transport::stdio()).await?;
    let _reason = running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use rmcp::ServerHandler as _;

    use super::*;

    #[test]
    fn instructions_mention_the_viewer() {
        let server = ViewerMcpServer::new(Url::parse("http://127.0.0.1:1234").ok());
        let instructions = server.instructions();
        assert!(instructions.starts_with(
            "This server was started for the Rerun viewer at `http://127.0.0.1:1234/`, but it is not connected yet"
        ));
        assert!(instructions.ends_with(INSTRUCTIONS));
        assert_eq!(ViewerMcpServer::new(None).instructions(), INSTRUCTIONS);
    }

    /// Snapshot of the documentation the llm will see when loading the mcp tools.
    ///
    /// It's useful to look at the snapshot output to check how much llm context the tool
    /// definitions will use.
    #[test]
    fn agent_surface_snapshot() {
        let server = ViewerMcpServer::new(None);

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
}
