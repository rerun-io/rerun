//! The viewer toolset: drives a running Rerun Viewer over its `ViewerControlService`.
//!
//! It owns the connection to the viewer, which the egui toolset (see [`crate::egui`]) also runs
//! over: each of its `egui_inspection` request/response exchanges is one `EguiInspect` call.
//!
//! The viewer-control tools (`viewer_get_state`, `viewer_set_time_cursor`, `viewer_close`, …) are
//! generated from `viewer_control.proto`: one tool per operation in the request envelope's
//! `oneof`. `viewer_connect` and `viewer_disconnect` are hand-written, since they manage the connection itself
//! rather than driving the viewer.
//!
//! The agent never sees gRPC, but it does read the `.proto` documentation: `re_viewer_control_tools`
//! derives each viewer-control tool's schema and description from the descriptor set, which
//! carries the comments.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use rmcp::{
    ErrorData as McpError,
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::{CallToolRequestParams, CallToolResult, ContentBlock, Tool},
    service::{Peer, RequestContext, RoleServer},
    tool, tool_router,
};
use serde::Deserialize;
use tonic::transport::Channel;
use url::Url;

use egui_inspection::protocol::{self, PROTOCOL_VERSION, Request, Response};
use egui_inspection_tools::{BoxFuture, Transport};
use re_protos::viewer_control::v1alpha1::{
    EguiInspectRequest, GetLogsRequest, GetStateRequest, ListCommandsRequest, RunCommandRequest,
    UnexpectedResponseKind, ViewerControlOp, ViewerLogEntry,
    viewer_control_service_client::ViewerControlServiceClient,
};
use re_viewer_control_tools::ScreenshotDelivery;

use crate::{ToolResult, annotations, complete, text_error};

const DEFAULT_VIEWER_ENDPOINT: &str = "http://127.0.0.1:9876";

/// The viewer command that re-reads a catalog server's entries ("Refresh server").
const REFRESH_SERVER_COMMAND: &str = "server.refresh";

/// Agents on MCP are coding agents that can read files. See [`ScreenshotDelivery::File`].
const SCREENSHOT_DELIVERY: ScreenshotDelivery = ScreenshotDelivery::File;

/// Appended to the description of every viewer-control tool, since they share one connection.
const REQUIRES_CONNECT: &str = "\n\nRequires `viewer_connect`.";

/// Deadline for one viewer-control operation.
///
/// Matches the viewer's own deadline for an answer, `re_log_channel::VIEWER_CONTROL_TIMEOUT`.
/// Slow work, like `open` waiting on a slow server, answers before that, so anything this
/// slow means the viewer is gone rather than busy.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Deadline for one `egui_inspection` exchange.
///
/// A UI request should be as quick as the interaction it stands for, so this allows only for
/// network hiccups and for the slow transfer of a screenshot. A viewer that takes longer is one
/// to fix rather than to wait for.
const UI_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Operating guidance for the viewer tools, part of the MCP `instructions`. The per-tool
/// descriptions cover each command in isolation; this establishes the cross-cutting workflow —
/// `viewer_connect` first — that an agent otherwise has to infer.
const INSTRUCTIONS: &str = include_str!("viewer_instructions.md");

/// Why the catalog server of the connected viewer is unknown.
#[derive(Debug, thiserror::Error)]
pub enum ViewerCatalogError {
    #[error("Not connected to a viewer")]
    NotConnected,

    #[error("Failed to read the viewer's state: {source}")]
    ReadState { source: ExecuteError },

    #[error("The viewer reported an invalid catalog URL: {source}. URL: {url:?}")]
    InvalidUrl { source: re_uri::Error, url: String },
}

/// Why a viewer-control operation got no usable answer.
#[derive(Debug, thiserror::Error)]
pub enum ExecuteError {
    #[error("The viewer did not answer within {REQUEST_TIMEOUT:?}")]
    Timeout,

    #[error("The ViewerControl request failed ({:?}): {}", .source.code(), .source.message())]
    Rpc { source: tonic::Status },

    #[error("{source}")]
    UnexpectedResponse { source: UnexpectedResponseKind },
}

/// Send one viewer-control operation over `ViewerControlService::ViewerControl` and unwrap the
/// matching response, so callers name a request type rather than the `oneof` variants.
async fn execute<Op: ViewerControlOp>(
    client: &mut ViewerControlServiceClient<Channel>,
    request: Op,
) -> Result<Op::Response, ExecuteError> {
    let response = tokio::time::timeout(
        REQUEST_TIMEOUT,
        client.viewer_control(request.into_envelope()),
    )
    .await
    .map_err(|_elapsed| ExecuteError::Timeout)?
    .map_err(|source| ExecuteError::Rpc { source })?
    .into_inner();
    Op::from_envelope(response).map_err(|source| ExecuteError::UnexpectedResponse { source })
}

/// An [`egui_inspection_tools::Transport`] that carries each `egui_inspection` request/response
/// over a unary `EguiInspect` gRPC call to the running viewer.
#[derive(Clone)]
struct GrpcInspector {
    client: ViewerControlServiceClient<Channel>,
}

impl Transport for GrpcInspector {
    // relint: allow(string_error): the signature of `egui_inspection_tools::Transport::request`
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
                     window stalls all of them. `viewer_get_state` and \
                     `viewer_get_logs` are answered without a frame and still work: use \
                     them to see whether the viewer is busy loading, and retry once it is idle."
                )
            })?
            .map_err(|err| format!("egui_inspect rpc failed: {err}"))?
            .into_inner();
            protocol::decode_body(&response.response).map_err(|err| err.to_string())
        })
    }
}

/// Why dialing the viewer failed.
#[derive(Debug, thiserror::Error)]
enum ConnectError {
    #[error("Invalid viewer endpoint: {source}")]
    InvalidEndpoint { source: tonic::transport::Error },

    #[error("{}", unreachable_message(endpoint, source))]
    Unreachable {
        endpoint: Url,
        source: tonic::transport::Error,
    },

    #[error("{0}")]
    Inspect(String),

    #[error("{message}")]
    Peer { message: String },

    #[error("unexpected response to GetInfo")]
    UnexpectedResponse,
}

fn unreachable_message(endpoint: &Url, err: &tonic::transport::Error) -> re_error::StructuredError {
    re_error::StructuredError::from_summary(
        "Nothing serves ViewerControl at this endpoint. \
         Is a viewer started with the `rerun` command running there?",
    )
    .with_detail(re_error::format_ref(err))
    .with_detail(format!("Endpoint: {endpoint}"))
}

/// Identity of the connected viewer, captured at connect time.
#[derive(Clone, Debug, serde::Serialize)]
struct PeerInfo {
    /// The gRPC endpoint dialed.
    endpoint: Url,

    /// The `egui_inspection` protocol version this server speaks.
    protocol_version: u32,

    /// The name the viewer gives itself, if any.
    label: Option<String>,
}

/// Dial the viewer and ask it who it is.
async fn connect_grpc(
    endpoint: &Url,
) -> Result<(PeerInfo, ViewerControlServiceClient<Channel>), ConnectError> {
    // No `Endpoint::timeout` here: that is one deadline for every RPC on the channel, and the
    // two kinds of call have very different expectations of how long the viewer may take.
    // Each call site applies its own instead.
    let channel = tonic::transport::Endpoint::from_shared(endpoint.to_string())
        .map_err(|source| ConnectError::InvalidEndpoint { source })?
        .connect_timeout(REQUEST_TIMEOUT)
        .connect()
        .await
        .map_err(|source| ConnectError::Unreachable {
            endpoint: endpoint.clone(),
            source,
        })?;
    let client = ViewerControlServiceClient::new(channel);
    let inspector = GrpcInspector {
        client: client.clone(),
    };

    // Read the peer's label up front (also a liveness check), matching the TCP `attach` path.
    let label = match inspector
        .request(Request::GetInfo)
        .await
        .map_err(ConnectError::Inspect)?
    {
        Response::Info { label, .. } => label,
        Response::Error { message } => return Err(ConnectError::Peer { message }),
        _ => return Err(ConnectError::UnexpectedResponse),
    };

    let peer = PeerInfo {
        endpoint: endpoint.clone(),
        protocol_version: PROTOCOL_VERSION,
        label,
    };
    Ok((peer, client))
}

/// The live connection to the viewer, established on `viewer_connect` and dropped on
/// `viewer_disconnect`.
struct Connection {
    client: ViewerControlServiceClient<Channel>,

    /// The Viewer gRPC endpoint this connection was dialed with.
    viewer_endpoint: Url,
    peer: PeerInfo,

    /// Sequence number of the last viewer log entry appended to a tool result.
    /// Starts at the newest entry at connect time, so old logs are not replayed.
    log_cursor: AtomicU64,

    /// Every command id the viewer knows, whether it can run right now or not.
    ///
    /// Asked for at connect time, so that `run_command` lists them. `None` if that request failed.
    command_ids: Option<Vec<String>>,
}

/// Prefix on every tool this toolset defines.
const VIEWER_PREFIX: &str = re_viewer_control_tools::TOOL_NAME_PREFIX;

/// Tools whose results cannot include viewer log entries.
const TOOLS_WITHOUT_LOG: &[&str] = &["viewer_disconnect"];

/// The viewer toolset: the connection tools and the viewer-control operations.
pub struct ViewerToolset {
    /// The active connection, `Some` while connected and `None` otherwise. Tool handlers clone
    /// the `Arc<Connection>` out of the lock (sync, so it can't be held across an await) and
    /// then use it freely.
    conn: Arc<Mutex<Option<Arc<Connection>>>>,

    /// Router for the hand-written tools.
    tool_router: ToolRouter<Self>,

    /// Set when the server was started for one specific viewer, which it connects to on startup.
    viewer_endpoint: Option<Url>,
}

/// Arguments for the `viewer_connect` tool.
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
impl ViewerToolset {
    pub fn new(viewer_endpoint: Option<Url>) -> Self {
        Self {
            conn: Arc::new(Mutex::new(None)),
            tool_router: Self::tool_router(),
            viewer_endpoint,
        }
    }

    /// What the egui toolset drives the connected viewer's widgets over, `None` while
    /// disconnected.
    pub fn ui_transport(&self) -> Option<Arc<dyn Transport>> {
        self.conn.lock().as_ref().map(|conn| {
            Arc::new(GrpcInspector {
                client: conn.client.clone(),
            }) as Arc<dyn Transport>
        })
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
        let (peer, mut client) = connect_grpc(endpoint)
            .await
            .map_err(|err| format!("connect failed: {err}"))?;
        let newest_log = execute(
            &mut client,
            GetLogsRequest {
                after_sequence: None,
            },
        )
        .await
        .map_or_else(
            |err| {
                re_log::warn!("Failed to read the viewer log; new tool results may repeat old log entries: {err}");
                0
            },
            |response| response.entries.last().map_or(0, |entry| entry.sequence),
        );
        let command_ids = execute(
            &mut client,
            ListCommandsRequest {
                include_unavailable: Some(true),
                ..Default::default()
            },
        )
        .await
        .inspect_err(|err| re_log::warn!("Failed to list the viewer's commands: {err}"))
        .ok()
        .map(|response| {
            response
                .commands
                .into_iter()
                .map(|command| command.id)
                .collect()
        });
        *self.conn.lock() = Some(Arc::new(Connection {
            client,
            viewer_endpoint: endpoint.clone(),
            peer: peer.clone(),
            log_cursor: AtomicU64::new(newest_log),
            command_ids,
        }));
        Ok(peer)
    }

    /// The connected viewer's command ids, if known. See [`Connection::command_ids`].
    fn command_ids(&self) -> Option<Vec<String>> {
        self.conn
            .lock()
            .as_ref()
            .and_then(|conn| conn.command_ids.clone())
    }

    /// Connect to the endpoint the toolset was started for, if any.
    ///
    /// A failure is only logged: the viewer may come up later, and `viewer_connect` still works.
    pub async fn connect_on_startup(&self) {
        let Some(endpoint) = &self.viewer_endpoint else {
            return;
        };
        match self.connect_to(endpoint).await {
            Ok(_) => re_log::info!("Connected to viewer at {endpoint}"),
            Err(err) => re_log::warn!("Failed to connect to the viewer: {err}"),
        }
    }

    /// Run one tool of this toolset, appending the viewer's new log entries to its result.
    pub async fn call(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let name = request.name.clone();
        let Some(ours) = name
            .strip_prefix(VIEWER_PREFIX)
            .filter(|name| {
                self.tool_router.has_route(name) || re_viewer_control_tools::is_operation(name)
            })
            .map(ToOwned::to_owned)
        else {
            return Ok(text_error(format!("unknown tool `{name}`")));
        };

        let mut result = if self.tool_router.has_route(&ours) {
            let mut request = request;
            request.name = std::borrow::Cow::Owned(ours);
            complete(
                self.tool_router
                    .call(ToolCallContext::new(self, request, context))
                    .await?,
            )
        } else {
            match self.call_operation(&ours, request.arguments).await {
                Ok(result) => result,
                Err(err) => text_error(err),
            }
        };

        if !TOOLS_WITHOUT_LOG.contains(&name.as_ref()) {
            self.append_new_logs(&mut result).await;
        }
        Ok(result)
    }

    /// The catalog server the connected viewer serves, if it serves one.
    pub async fn catalog_url(&self) -> Result<Option<re_uri::Origin>, ViewerCatalogError> {
        let conn = self
            .conn
            .lock()
            .clone()
            .ok_or(ViewerCatalogError::NotConnected)?;
        let mut client = conn.client.clone();
        let state = execute(&mut client, GetStateRequest {})
            .await
            .map_err(|source| ViewerCatalogError::ReadState { source })?;
        state
            .catalog_url
            .map(|url| {
                url.parse()
                    .map_err(|source| ViewerCatalogError::InvalidUrl { source, url })
            })
            .transpose()
    }

    // TODO(RR-4859, RR-4853): remove once the catalog server sends an event for every change.
    /// Has the connected viewer refresh the catalog server at `origin`, so that the user sees a
    /// change the agent made. Does nothing if the viewer is not connected to that server.
    pub async fn show_catalog_changes(&self, origin: &re_uri::Origin) {
        let Some(conn) = self.conn.lock().clone() else {
            return;
        };
        let mut client = conn.client.clone();
        let request = RunCommandRequest {
            id: REFRESH_SERVER_COMMAND.to_owned(),
            store_id: None,
            server_origin: Some(origin.to_string()),
            allow_blocking_native_modal: None,
        };
        if let Err(err) = execute(&mut client, request).await {
            re_log::debug!("Failed to refresh the changed catalog in the viewer: {err}");
        }
    }

    /// Whether the connected viewer runs on this machine.
    pub fn is_on_this_machine(&self) -> bool {
        self.conn
            .lock()
            .as_ref()
            .is_some_and(|conn| match conn.viewer_endpoint.host() {
                Some(url::Host::Domain(domain)) => domain == "localhost",
                Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
                Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
                None => false,
            })
    }

    /// Appends the viewer log entries since the previous tool call to `result`,
    /// so the agent sees warnings and errors as they happen.
    pub async fn append_new_logs(&self, result: &mut CallToolResult) {
        let conn = self.conn.lock().clone();
        let Some(conn) = conn else {
            return;
        };
        let after_sequence = conn.log_cursor.load(Ordering::Relaxed);
        let mut client = conn.client.clone();
        let response = match execute(
            &mut client,
            GetLogsRequest {
                after_sequence: Some(after_sequence),
            },
        )
        .await
        {
            Ok(response) => response,
            Err(err) => {
                re_log::warn!("Failed to read the viewer log: {err}");
                result.content.push(ContentBlock::text(format!(
                    "Could not read the viewer log: {err}"
                )));
                return;
            }
        };
        let entries = response.entries;
        let Some(last) = entries.last() else {
            return;
        };
        conn.log_cursor.store(last.sequence, Ordering::Relaxed);
        result.content.push(ContentBlock::text(format!(
            "Viewer log since the previous tool call:\n{}",
            format_log_entries(&entries)
        )));
    }

    /// Every tool of this toolset: the connection tools, and the operations generated from
    /// `viewer_control.proto`.
    ///
    /// They stay listed even while disconnected. Only `run_command` depends on the connection:
    /// it lists the connected viewer's command ids.
    pub fn tools(&self) -> Vec<Tool> {
        let command_ids = self.command_ids();
        let mut tools = self.tool_router.list_all();
        tools.extend(
            re_viewer_control_tools::tools(SCREENSHOT_DELIVERY, command_ids.as_deref())
                .into_iter()
                .map(
                    |re_agent_tools::ToolDefinition {
                         name,
                         description,
                         input_schema,
                         effect,
                         open_world,
                     }| {
                        Tool::new(
                            name,
                            format!("{description}{REQUIRES_CONNECT}"),
                            Arc::new(input_schema),
                        )
                        .with_annotations(annotations(effect, open_world))
                    },
                ),
        );
        for tool in &mut tools {
            tool.name = std::borrow::Cow::Owned(format!("{VIEWER_PREFIX}{}", tool.name));
        }
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
        let request = re_viewer_control_tools::build_request(name, arguments, SCREENSHOT_DELIVERY)
            // relint: allow(map_err_to_string): `ToolError` is a `String`, which `rmcp` shows to the agent
            .map_err(|err| err.to_string())?;
        let mut client = self.client()?;
        let response = client
            .viewer_control(request)
            .await
            .map_err(|err| format!("{name} failed: {err}"))?
            .into_inner();
        let json = re_viewer_control_tools::response_json(name, &response)
            .map_err(|err| format!("{name} failed: {err}"))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            json.to_string(),
        )]))
    }

    /// The viewer part of the MCP `instructions`: [`INSTRUCTIONS`], led by the configured viewer's
    /// connection state.
    pub fn instructions(&self) -> String {
        match &self.viewer_endpoint {
            Some(endpoint) if self.conn.lock().is_some() => format!(
                "This server was started for the Rerun viewer at `{endpoint}` and is already connected to it. \
                 Do not call `viewer_connect`; start using the other tools right away.\n\n{INSTRUCTIONS}"
            ),
            Some(endpoint) => format!(
                "This server was started for the Rerun viewer at `{endpoint}`, but it is not connected yet. \
                 Call `viewer_connect` before using the other tools.\n\n{INSTRUCTIONS}"
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
            .ok_or_else(|| "not connected — call `viewer_connect` first".to_owned())
    }

    /// Connect to a running Rerun viewer over gRPC.
    ///
    /// The other tools will be available once the connection is established.
    /// `endpoint` defaults to the viewer this server was started for, else `http://127.0.0.1:9876`
    /// (the viewer's default gRPC address).
    /// Connecting again to the same endpoint is a no-op. Connecting to a different endpoint
    /// replaces the old connection once the new connection succeeds. Call `viewer_disconnect` to drop
    /// the connection without replacing it.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn connect(
        &self,
        Parameters(args): Parameters<ConnectArgs>,
        client: Peer<RoleServer>,
    ) -> ToolResult<CallToolResult> {
        let endpoint = args
            .endpoint
            .or_else(|| self.viewer_endpoint.clone())
            .map_or_else(
                // relint: allow(map_err_to_string): `ToolError` is a `String`, which `rmcp` shows to the agent
                || Url::parse(DEFAULT_VIEWER_ENDPOINT).map_err(|err| err.to_string()),
                Ok,
            )?;
        let command_ids_before = self.command_ids();
        let peer = self.connect_to(&endpoint).await?;
        if self.command_ids() != command_ids_before {
            notify_tool_list_changed(&client).await;
        }
        Ok(CallToolResult::structured(serde_json::json!({
            "ok": true,
            "connected": endpoint,
            "peer": peer,
        })))
    }

    /// Disconnect from the viewer.
    /// The tools stop working until `viewer_connect` is called again.
    #[tool(annotations(
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn disconnect(
        &self,
        Parameters(_args): Parameters<EmptyArgs>,
        client: Peer<RoleServer>,
    ) -> ToolResult<CallToolResult> {
        let conn = self.conn.lock().take();
        if let Some(conn) = conn {
            if conn.command_ids.is_some() {
                notify_tool_list_changed(&client).await;
            }
            Ok(CallToolResult::structured(
                serde_json::json!({ "ok": true }),
            ))
        } else {
            Err("not connected".to_owned())
        }
    }
}

/// Tell the agent to list the tools again, since `run_command` now lists other command ids.
async fn notify_tool_list_changed(client: &Peer<RoleServer>) {
    client
        .notify_tool_list_changed()
        .await
        .inspect_err(|err| re_log::warn!("Failed to tell the agent its tools changed: {err}"))
        .ok();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instructions_mention_the_viewer() {
        let viewer = ViewerToolset::new(Url::parse("http://127.0.0.1:1234").ok());
        let instructions = viewer.instructions();
        assert!(instructions.starts_with(
            "This server was started for the Rerun viewer at `http://127.0.0.1:1234/`, but it is not connected yet"
        ));
        assert!(instructions.ends_with(INSTRUCTIONS));
        assert_eq!(ViewerToolset::new(None).instructions(), INSTRUCTIONS);
    }

    #[tokio::test]
    async fn a_viewer_without_viewer_control_gets_an_actionable_error() {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .map(|addr| addr.port())
            .expect("a free port");
        let endpoint = Url::parse(&format!("http://127.0.0.1:{port}")).expect("a valid URL");

        let err = ViewerToolset::new(None)
            .connect_to(&endpoint)
            .await
            .expect_err("nothing listens on the port");
        assert!(
            err.contains("refused"),
            "The error keeps its root cause, not just \"transport error\": {err}"
        );
        assert!(
            err.contains("Is a viewer started with the `rerun` command running there?"),
            "The error says how to fix it: {err}"
        );
        assert!(
            err.ends_with(&format!("\n- Endpoint: {endpoint}")),
            "The endpoint is the last detail: {err}"
        );
    }

    #[test]
    fn instructions_name_available_tools() {
        let tools = ViewerToolset::new(None).tools();
        let names = crate::tests::backticked_names(INSTRUCTIONS)
            .filter(|name| name.starts_with(VIEWER_PREFIX));
        for name in names {
            assert!(
                tools.iter().any(|tool| tool.name == name),
                "unknown tool: {name}"
            );
        }
    }
}
