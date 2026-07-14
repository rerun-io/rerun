//! `re_viewer_mcp` — an MCP server that drives the Rerun viewer.
//!
//! It reuses the full `egui_mcp` UI tool set (`query_tree`, `screenshot`, `click`, …) but, instead of
//! dialing a local inspection socket, it drives the viewer over rerun's gRPC `ViewerControlService`.
//!
//! Each egui tool call becomes one `egui_inspection` request/response exchange, carried by a single
//! `Inspect` RPC.
//!
//! The server is exposed two ways — the standalone `re-viewer-mcp` binary and the `rerun viewer-mcp`
//! CLI subcommand.

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt as _,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::{
        CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo,
    },
    schemars,
    service::{RequestContext, RoleServer},
    tool, tool_router, transport,
};
use serde::{Deserialize, Serialize};
use tonic::transport::Channel;

use egui_inspection::protocol::{self, PROTOCOL_VERSION, Request, Response};
use egui_mcp::{BoxFuture, Bridge, PeerInfo, Transport, UiServer};
use re_protos::common::v1alpha1::{
    ApplicationId, StoreId, StoreKind, TimeRange, TimeType, Timeline,
};
use re_protos::sdk_comms::v1alpha1::{
    GetViewerStateRequest, GetViewerStateResponse, InspectRequest, OpenUrlRequest,
    SetTimeCursorRequest, SetTimeCursorResponse, TimeCursor, ViewerRecording, ViewerTimeline,
    viewer_control_service_client::ViewerControlServiceClient,
};

const DEFAULT_VIEWER_ENDPOINT: &str = "http://127.0.0.1:9876";

/// Per-request RPC deadline.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// An [`egui_mcp::Transport`] that carries each `egui_inspection` request/response over a unary
/// `Inspect` gRPC call to the running viewer.
#[derive(Clone)]
struct GrpcInspector {
    client: ViewerControlServiceClient<Channel>,
}

impl Transport for GrpcInspector {
    fn request(&self, req: Request) -> BoxFuture<'_, Result<Response, String>> {
        Box::pin(async move {
            let request = protocol::encode_body(&req).map_err(|err| err.to_string())?;
            let mut client = self.client.clone();
            let response = client
                .inspect(InspectRequest { request })
                .await
                .map_err(|err| format!("inspect rpc failed: {err}"))?
                .into_inner();
            protocol::decode_body(&response.response).map_err(|err| err.to_string())
        })
    }
}

/// Dial the viewer once and build both the [`Bridge`] (which tunnels the egui tools over the
/// unary `Inspect` RPC) and the gRPC client the rerun-specific tools call through — sharing the
/// single connection between them.
async fn connect_grpc(
    endpoint: &str,
) -> Result<(Bridge, ViewerControlServiceClient<Channel>), String> {
    let channel = tonic::transport::Endpoint::from_shared(endpoint.to_owned())
        .map_err(|err| err.to_string())?
        .timeout(REQUEST_TIMEOUT)
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
            transport: endpoint.to_owned(),
            protocol_version: PROTOCOL_VERSION,
            label,
        },
    );
    Ok((bridge, client))
}

/// The live connection to the viewer: the egui [`UiServer`] (which tunnels the egui tools over
/// the `Inspect` RPC) and the raw gRPC client (used by the rerun-specific tools). Both are
/// established together on `connect` and dropped together on `disconnect`, so they live behind a
/// single lock.
struct Connection {
    ui: UiServer,
    client: ViewerControlServiceClient<Channel>,
}

/// The `re_viewer_mcp` server: rerun-specific connection / state tools, plus the reusable `egui_mcp`
/// [`UiServer`], built on `connect` and dropped on `disconnect`, that drives the live viewer.
#[derive(Clone)]
struct ViewerMcpServer {
    /// The active connection, `Some` while connected and `None` otherwise. Tool handlers clone
    /// the `Arc<Connection>` out of the lock (sync, so it can't be held across an await) and
    /// then use it freely.
    conn: Arc<Mutex<Option<Arc<Connection>>>>,

    /// Router over the egui UI/inspection tools. Independent of the connection, so the tools
    /// stay listed while disconnected; a call before `connect` returns `no app connected`.
    ui_router: ToolRouter<UiServer>,

    /// Router for the rerun-specific tools layered on top of the egui ones.
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct ConnectArgs {
    /// gRPC endpoint of the running viewer's `ViewerControlService`.
    /// Defaults to `http://127.0.0.1:9876`.
    #[serde(default)]
    endpoint: Option<String>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct EmptyArgs {}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct SetTimeArgs {
    /// Recording to seek (see `viewer_state`).
    /// Defaults to the active recording.
    #[serde(default)]
    store_id: Option<StoreIdArg>,

    /// Timeline to seek on (see `viewer_state` for available timelines).
    /// Defaults to the active timeline.
    #[serde(default)]
    timeline: Option<String>,

    /// Time to seek to: a sequence index for sequence timelines, or nanoseconds for temporal timelines (see each timeline's `type` and `min`/`max` in `viewer_state`).
    time: i64,

    /// If true, start playing the recording from the new time cursor position instead of just
    /// moving the cursor and staying paused. Defaults to false.
    #[serde(default)]
    play: bool,
}

/// JSON representation of a proto [`StoreId`], used both as agent-facing output (see
/// `viewer_state`) and as tool input identifying a recording to target.
#[derive(Debug, Default, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct StoreIdArg {
    /// The kind of store: `"recording"`, `"blueprint"`, or `"unspecified"`.
    kind: String,

    /// The recording id.
    recording_id: String,

    /// The application id the recording belongs to.
    application_id: String,
}

impl From<StoreId> for StoreIdArg {
    fn from(store_id: StoreId) -> Self {
        let kind = match store_id.kind() {
            StoreKind::Unspecified => "unspecified",
            StoreKind::Recording => "recording",
            StoreKind::Blueprint => "blueprint",
        };
        Self {
            kind: kind.to_owned(),
            recording_id: store_id.recording_id,
            application_id: store_id.application_id.unwrap_or_default().id,
        }
    }
}

impl From<StoreIdArg> for StoreId {
    fn from(arg: StoreIdArg) -> Self {
        let StoreIdArg {
            kind,
            recording_id,
            application_id,
        } = arg;
        let kind = match kind.as_str() {
            "blueprint" => StoreKind::Blueprint,
            "recording" => StoreKind::Recording,
            _ => StoreKind::Unspecified,
        };
        Self {
            kind: kind as i32,
            recording_id,
            application_id: (!application_id.is_empty())
                .then_some(ApplicationId { id: application_id }),
        }
    }
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
struct OpenUrlArgs {
    /// The URL to open in the viewer: a recording/blueprint file URL, a `rerun://` dataset URI, a redap server/catalog URL, or an intra-recording link.
    url: String,
}

#[tool_router]
impl ViewerMcpServer {
    fn new() -> Self {
        Self {
            conn: Arc::new(Mutex::new(None)),
            ui_router: UiServer::router(),
            tool_router: Self::tool_router(),
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
            .ok_or_else(|| "not connected — call `connect` first".to_owned())
    }

    /// Connect to a running Rerun viewer over gRPC. The other tools will be available once the connection is established.
    /// `endpoint` defaults to `http://127.0.0.1:9876` (the viewer's default gRPC address).
    /// Call `disconnect` to drop the connection.
    #[tool]
    async fn connect(
        &self,
        Parameters(args): Parameters<ConnectArgs>,
    ) -> ToolResult<CallToolResult> {
        if self.conn.lock().is_some() {
            return Err(
                "already connected — call `disconnect` first to drop the current connection"
                    .to_owned(),
            );
        }
        let endpoint = args
            .endpoint
            .unwrap_or_else(|| DEFAULT_VIEWER_ENDPOINT.to_owned());
        let (bridge, client) = connect_grpc(&endpoint)
            .await
            .map_err(|err| format!("connect failed: {err}"))?;
        let peer = bridge.peer_info.clone();
        *self.conn.lock() = Some(Arc::new(Connection {
            ui: UiServer::new(bridge),
            client,
        }));
        Ok(CallToolResult::structured(serde_json::json!({
            "ok": true,
            "connected": endpoint,
            "peer": peer,
        })))
    }

    /// Disconnect from the viewer, dropping the gRPC-backed bridge.
    /// The tools stop working until `connect` is called again.
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

    /// Report the current Rerun viewer state as JSON: the active recording, the current page URL, and every open recording (recording id, application id) with its timelines, their time ranges, and its current time cursor.
    /// Use this to learn which recording/timeline to drive and what time values are valid before calling `set_time`.
    /// Requires `connect`.
    #[tool]
    async fn viewer_state(
        &self,
        Parameters(_args): Parameters<EmptyArgs>,
    ) -> ToolResult<CallToolResult> {
        let mut client = self.client()?;
        let response = client
            .get_viewer_state(GetViewerStateRequest {})
            .await
            .map_err(|err| format!("viewer_state failed: {err}"))?
            .into_inner();
        Ok(CallToolResult::success(vec![Content::text(
            viewer_state_to_json(response).to_string(),
        )]))
    }

    /// Set the time cursor (timeline position) of a recording in the Rerun viewer.
    /// `time` is a sequence index for sequence timelines or nanoseconds for temporal timelines (call `viewer_state` first for each timeline's type and valid range).
    /// `store_id` and `timeline` default to the active recording / active timeline.
    /// If `play` is unset or `false`, the recording will be paused. If `true`, the recording will play from the selected time.
    /// Requires `connect`.
    #[tool]
    async fn set_time(
        &self,
        Parameters(args): Parameters<SetTimeArgs>,
    ) -> ToolResult<CallToolResult> {
        let mut client = self.client()?;
        let response = client
            .set_time_cursor(SetTimeCursorRequest {
                store_id: args.store_id.map(StoreId::from),
                timeline: args.timeline.map(|name| Timeline { name }),
                time: Some(args.time.into()),
                play: args.play,
            })
            .await
            .map_err(|err| format!("set_time failed: {err}"))?
            .into_inner();
        Ok(CallToolResult::success(vec![Content::text(
            set_time_to_json(response).to_string(),
        )]))
    }

    /// Open a URL in the Rerun viewer.
    /// The URL can be a recording/blueprint file URL, a `rerun://` dataset URI, a redap server/catalog URL, or an intra-recording link.
    /// Requires `connect`.
    #[tool]
    async fn open_url(
        &self,
        Parameters(args): Parameters<OpenUrlArgs>,
    ) -> ToolResult<CallToolResult> {
        let mut client = self.client()?;
        client
            .open_url(OpenUrlRequest {
                url: args.url.clone(),
            })
            .await
            .map_err(|err| format!("open_url failed: {err}"))?;
        Ok(CallToolResult::structured(serde_json::json!({
            "ok": true,
            "opened": args.url,
        })))
    }
}

/// Timeline name from a proto [`Timeline`].
fn timeline_name(timeline: &Timeline) -> &str {
    let Timeline { name } = timeline;
    name
}

/// Friendly name for a [`TimeType`] stored as its raw `i32` proto representation.
fn time_type_name(raw: i32) -> String {
    TimeType::try_from(raw)
        .unwrap_or(TimeType::Unspecified)
        .to_string()
}

/// Render a [`GetViewerStateResponse`] as the JSON object surfaced to the agent.
///
/// Unfortunately our protos don't have a serde implementation so we manually convert it for now.
fn viewer_state_to_json(state: GetViewerStateResponse) -> serde_json::Value {
    use serde_json::json;
    let GetViewerStateResponse {
        active_store_id,
        url,
        recordings,
    } = state;
    json!({
        "active_store_id": active_store_id.map(StoreIdArg::from),
        "url": url,
        "recordings": recordings
            .into_iter()
            .map(|ViewerRecording {
                    store_id,
                    timelines,
                    current_time,
                }| {
                json!({
                    "store_id": store_id.map(StoreIdArg::from),
                    "timelines": timelines
                        .iter()
                        .map(|ViewerTimeline {
                                timeline,
                                time_type,
                                time_range,
                            }| {
                            let (min, max) = match time_range {
                                Some(TimeRange { start, end }) => (Some(*start), Some(*end)),
                                None => (None, None),
                            };
                            json!({
                                "timeline": timeline.as_ref().map(timeline_name),
                                "type": time_type_name(*time_type),
                                "min": min,
                                "max": max,
                            })
                        })
                        .collect::<Vec<_>>(),
                    "current_time": current_time.as_ref().map(time_cursor_to_json),
                })
            })
            .collect::<Vec<_>>(),
    })
}

/// Render a [`TimeCursor`] as the JSON object surfaced to the agent.
fn time_cursor_to_json(cursor: &TimeCursor) -> serde_json::Value {
    let TimeCursor {
        timeline,
        time_type,
        time,
    } = cursor;
    serde_json::json!({
        "timeline": timeline.as_ref().map(timeline_name),
        "type": time_type.map(time_type_name),
        "time": time.as_ref().map(|t| t.time),
    })
}

/// Render a [`SetTimeCursorResponse`] as the JSON object surfaced to the agent (exhaustively
/// destructured, see [`viewer_state_to_json`]).
fn set_time_to_json(response: SetTimeCursorResponse) -> serde_json::Value {
    let SetTimeCursorResponse {
        store_id,
        timeline,
        time_type,
        time,
    } = response;
    serde_json::json!({
        "store_id": store_id.map(StoreIdArg::from),
        "timeline": timeline.as_ref().map(timeline_name),
        "type": time_type_name(time_type),
        "time": time.map(|t| t.time),
    })
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
/// `connect` first, then the observe→act→verify loop the egui tools share — that an agent
/// otherwise has to infer.
const INSTRUCTIONS: &str = r#"This MCP drives a live Rerun viewer: it reads the viewer's accessibility tree and synthesizes real input events. Work in an observe → act → verify loop.

Getting oriented:
- Call `connect` first (it dials the viewer's gRPC server); every other tool errors until then.
- If no viewer is running, launch one. If the user tells you to work in the background, or no desktop is available, use `--headless`.
- Start most tasks with `query_tree` to discover widgets and their ids, and/or `screenshot` to see the rendered frame.

Targeting widgets:
- Prefer locators — an `id` from `query_tree`, or `role`/`label_contains` — over a raw `pos`. Locators resolve to the widget's current position and survive layout changes; reach for `pos` only when nothing matches.

Acting and verifying:
- After an action that changes the UI, confirm it landed: `query_tree` for the expected state, `screenshot` to look, or `wait_for` to poll until async or animated UI settles.
- Use `batch` to act and observe in one round trip (e.g. `click` then `screenshot`), avoiding an extra turn.
- To move through time, call `viewer_state` for the recordings/timelines and their valid ranges, then `set_time`.

Conventions:
- Everything is in logical points, one shared coordinate frame: raw `pos`, `resize` dimensions, the `bounds` from `query_tree`/`get_node`, and a default (`pixels_per_point: 1.0`) `screenshot`. So a node's `bounds` center is exactly where to `click`, and a pixel in the screenshot is a logical point. There is no fixed screen size; use `resize` to set the viewport."#;

impl ServerHandler for ViewerMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("viewer-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        // The rerun-specific tools plus the reusable egui UI tools. The egui router is
        // independent of the connection, so its tools stay listed even while disconnected.
        let mut tools = self.tool_router.list_all();
        tools.extend(self.ui_router.list_all());
        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        // Rerun-specific tools run on `self`; everything else is delegated to the attached UI
        // server, which exists only while connected.
        if self.tool_router.has_route(&request.name) {
            return self
                .tool_router
                .call(ToolCallContext::new(self, request, context))
                .await;
        }
        let conn = self.conn.lock().clone();
        let Some(conn) = conn else {
            return Ok(text_error("no app connected — call `connect` first"));
        };
        conn.ui.dispatch(&self.ui_router, request, context).await
    }
}

/// Serve the MCP server over stdio until the client disconnects.
///
/// Assumes the caller has already set up a Tokio runtime (this must run inside one) and logging.
/// Both the `rerun viewer-mcp` subcommand and the standalone `re-viewer-mcp` binary call this — each sets up
/// its own runtime and logging first.
pub async fn serve() -> anyhow::Result<()> {
    let server = ViewerMcpServer::new();
    let running = server.serve(transport::stdio()).await?;
    let _reason = running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use rmcp::ServerHandler as _;

    use super::*;

    /// Snapshot of the documentation the llm will see when loading the mcp tools.
    ///
    /// It's useful to look at the snapshot output to check how much llm context the tool
    /// definitions will use.
    #[test]
    fn agent_surface_snapshot() {
        let server = ViewerMcpServer::new();

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

        // The rerun-specific tools plus the reusable egui UI tools — the same set `list_tools`
        // serves.
        let mut tools = server.tool_router.list_all();
        tools.extend(server.ui_router.list_all());
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        for tool in &tools {
            write!(surface, "\n## {}\n\n", tool.name).unwrap();
            surface.push_str(&serde_json::to_string_pretty(tool).expect("serialize tool"));
            surface.push('\n');
        }

        insta::assert_snapshot!("agent_surface", surface);
    }
}
