use std::path::PathBuf;
use std::sync::Arc;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    AgentCapabilities, AuthMethod, AuthMethodId, AuthenticateRequest, CancelNotification,
    ContentBlock, CurrentModeUpdate, Implementation, InitializeRequest, McpServer, McpServerStdio,
    NewSessionRequest, PromptRequest, RequestPermissionRequest, RequestPermissionResponse,
    SessionId, SessionModeId, SessionModeState, SessionNotification, SessionUpdate,
    SetSessionModeRequest, StopReason,
};
use agent_client_protocol::{
    AcpAgent, AcpAgentConfig, Agent, Client, ConnectionTo, Error, LineDirection, Responder,
};
use futures::StreamExt as _;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};

/// An MCP server the agent spawns itself over stdio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpStdioServer {
    /// The name the agent sees the server under.
    pub name: String,

    /// Executable the agent runs to start the server.
    pub command: PathBuf,

    /// Arguments passed to [`Self::command`].
    pub args: Vec<String>,
}

/// Everything needed to spawn an agent and open a session with it.
#[derive(Clone, Debug)]
pub struct LaunchConfig {
    /// The agent executable, or an adapter such as `npx`.
    pub command: PathBuf,

    /// Arguments passed to [`Self::command`].
    pub args: Vec<String>,

    /// Environment variables set for the agent process, on top of the inherited ones.
    pub env: Vec<(String, String)>,

    /// Working directory of the agent process and of the session.
    pub cwd: PathBuf,

    /// Extra directories the agent may read and edit, e.g. one holding skills.
    pub additional_directories: Vec<PathBuf>,

    /// MCP servers the agent spawns and connects to when the session opens.
    pub mcp_servers: Vec<McpStdioServer>,

    /// Forward every JSON-RPC line as [`AgentEvent::Log`]. Stderr is always forwarded.
    pub log_protocol: bool,

    /// Markdown sent together with the first prompt of the session.
    pub preamble: Option<String>,
}

/// Something the UI wants the agent to do.
#[derive(Debug)]
pub enum AgentCommand {
    Prompt(Vec<ContentBlock>),
    Cancel,
    SetMode(SessionModeId),
    Authenticate(AuthMethodId),
    Shutdown,
}

/// Something that happened on the agent side, delivered to the UI thread.
#[derive(Debug)]
pub enum AgentEvent {
    /// Human-readable progress while connecting.
    Status(String),

    Initialized {
        agent_info: Option<Implementation>,
        capabilities: Box<AgentCapabilities>,
        auth_methods: Vec<AuthMethod>,
    },

    SessionStarted {
        session_id: SessionId,
        modes: Option<SessionModeState>,
    },

    /// The agent refused to start a session until the user authenticates.
    AuthRequired {
        methods: Vec<AuthMethod>,
        message: String,
    },

    Update(SessionUpdate),

    /// The agent wants the user to pick one of the options.
    /// The UI must answer via the responder, or the agent stays blocked.
    PermissionRequest {
        request: RequestPermissionRequest,
        responder: Responder<RequestPermissionResponse>,
    },

    TurnFinished(Result<StopReason, String>),

    Log {
        direction: LineDirection,
        line: String,
    },

    Error(String),

    /// The agent process is gone. No more events will follow.
    Disconnected,
}

/// Queues events for the UI thread and wakes it.
#[derive(Clone)]
struct EventSender {
    tx: std::sync::mpsc::Sender<AgentEvent>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl EventSender {
    fn send(&self, event: AgentEvent) {
        if self.tx.send(event).is_ok() {
            (self.wake)();
        }
    }
}

/// A running agent subprocess plus the ACP connection to it.
///
/// All protocol work happens on a dedicated thread. Dropping this asks the agent to shut down.
pub struct AgentConnection {
    command_tx: UnboundedSender<AgentCommand>,
    event_rx: std::sync::mpsc::Receiver<AgentEvent>,
}

impl AgentConnection {
    /// Spawns the agent. `wake` is called after every event so the UI can repaint.
    pub fn spawn(config: LaunchConfig, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let (command_tx, command_rx) = futures::channel::mpsc::unbounded();
        // Unbounded on purpose: events are small, the UI drains them every frame, and blocking
        // the protocol thread would also stall permission responders the agent is waiting on.
        #[expect(clippy::disallowed_methods)]
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let events = EventSender {
            tx: event_tx,
            wake: Arc::new(wake),
        };

        let spawned = std::thread::Builder::new()
            .name("re_agent_ui-acp".to_owned())
            .spawn(move || run(config, command_rx, &events));
        if let Err(err) = spawned {
            re_log::error!("Failed to spawn agent thread: {err}");
        }

        Self {
            command_tx,
            event_rx,
        }
    }

    pub fn send(&self, command: AgentCommand) {
        if self.command_tx.unbounded_send(command).is_err() {
            re_log::debug!("Agent is gone; dropping command");
        }
    }

    /// Drains all events that arrived since the last call.
    pub fn poll_events(&self) -> impl Iterator<Item = AgentEvent> + '_ {
        self.event_rx.try_iter()
    }
}

impl Drop for AgentConnection {
    fn drop(&mut self) {
        self.command_tx.unbounded_send(AgentCommand::Shutdown).ok();
    }
}

fn run(config: LaunchConfig, command_rx: UnboundedReceiver<AgentCommand>, events: &EventSender) {
    let agent_config = AcpAgentConfig::new(config.command.clone())
        .args(config.args.iter().cloned())
        .envs(config.env.iter().cloned());

    let log_protocol = config.log_protocol;
    let log_events = events.clone();
    let agent = AcpAgent::new(agent_config).with_debug(move |line, direction| {
        if log_protocol || direction == LineDirection::Stderr {
            log_events.send(AgentEvent::Log {
                direction,
                line: line.to_owned(),
            });
        }
    });

    let notification_events = events.clone();
    let permission_events = events.clone();
    let foreground_events = events.clone();

    let result = futures::executor::block_on(
        Client
            .builder()
            .name("re_agent_ui")
            .on_receive_notification(
                async move |notification: SessionNotification, _cx| {
                    notification_events.send(AgentEvent::Update(notification.update));
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .on_receive_request(
                async move |request: RequestPermissionRequest, responder, _cx| {
                    permission_events.send(AgentEvent::PermissionRequest { request, responder });
                    Ok(())
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_with(agent, |cx: ConnectionTo<Agent>| async move {
                drive(cx, config, command_rx, foreground_events).await
            }),
    );

    if let Err(err) = result {
        events.send(AgentEvent::Error(format!(
            "Agent connection failed: {}",
            describe_error(&err)
        )));
    }
    events.send(AgentEvent::Disconnected);
}

/// The foreground of the connection: initialize, open a session, then serve UI commands.
async fn drive(
    cx: ConnectionTo<Agent>,
    config: LaunchConfig,
    mut command_rx: UnboundedReceiver<AgentCommand>,
    events: EventSender,
) -> Result<(), Error> {
    events.send(AgentEvent::Status("Initializing…".to_owned()));

    let client_info = Implementation::new("re_agent_ui", env!("CARGO_PKG_VERSION"));
    let init = cx
        .send_request(InitializeRequest::new(ProtocolVersion::V1).client_info(client_info))
        .block_task()
        .await?;

    events.send(AgentEvent::Initialized {
        agent_info: init.agent_info.clone(),
        capabilities: Box::new(init.agent_capabilities.clone()),
        auth_methods: init.auth_methods.clone(),
    });

    let mut session_id = start_session(&cx, &config, &events, &init.auth_methods).await;

    while let Some(command) = command_rx.next().await {
        match command {
            AgentCommand::Prompt(prompt) => {
                let Some(session_id) = &session_id else {
                    events.send(AgentEvent::Error("No active session".to_owned()));
                    continue;
                };
                let events = events.clone();
                cx.send_request(PromptRequest::new(session_id.clone(), prompt))
                    .on_receiving_result(move |result| async move {
                        let result = result
                            .map(|response| response.stop_reason)
                            .map_err(|err| describe_error(&err));
                        events.send(AgentEvent::TurnFinished(result));
                        Ok(())
                    })?;
            }

            AgentCommand::Cancel => {
                if let Some(session_id) = &session_id {
                    cx.send_notification(CancelNotification::new(session_id.clone()))?;
                }
            }

            AgentCommand::SetMode(mode_id) => {
                let Some(session_id) = &session_id else {
                    continue;
                };
                let events = events.clone();
                let request = SetSessionModeRequest::new(session_id.clone(), mode_id.clone());
                cx.send_request(request)
                    .on_receiving_result(move |result| async move {
                        match result {
                            // Agents need not announce the mode they were just told to use,
                            // so report it ourselves once they have accepted it.
                            Ok(_) => events.send(AgentEvent::Update(
                                SessionUpdate::CurrentModeUpdate(CurrentModeUpdate::new(mode_id)),
                            )),
                            Err(err) => events.send(AgentEvent::Error(format!(
                                "Failed to set mode: {}",
                                describe_error(&err)
                            ))),
                        }
                        Ok(())
                    })?;
            }

            AgentCommand::Authenticate(method_id) => {
                events.send(AgentEvent::Status("Authenticating…".to_owned()));
                match cx
                    .send_request(AuthenticateRequest::new(method_id))
                    .block_task()
                    .await
                {
                    Ok(_) => {
                        session_id = start_session(&cx, &config, &events, &init.auth_methods).await;
                    }
                    Err(err) => {
                        events.send(AgentEvent::Error(format!(
                            "Authentication failed: {}",
                            describe_error(&err)
                        )));
                    }
                }
            }

            AgentCommand::Shutdown => break,
        }
    }

    Ok(())
}

async fn start_session(
    cx: &ConnectionTo<Agent>,
    config: &LaunchConfig,
    events: &EventSender,
    auth_methods: &[AuthMethod],
) -> Option<SessionId> {
    events.send(AgentEvent::Status("Starting session…".to_owned()));

    let mcp_servers = config
        .mcp_servers
        .iter()
        .map(|server| {
            let McpStdioServer {
                name,
                command,
                args,
            } = server.clone();
            McpServer::Stdio(McpServerStdio::new(name, command).args(args))
        })
        .collect();

    let request = NewSessionRequest::new(config.cwd.clone())
        .additional_directories(config.additional_directories.clone())
        .mcp_servers(mcp_servers);

    match cx.send_request(request).block_task().await {
        Ok(response) => {
            events.send(AgentEvent::SessionStarted {
                session_id: response.session_id.clone(),
                modes: response.modes,
            });
            Some(response.session_id)
        }
        Err(err) if err.code == Error::auth_required().code => {
            events.send(AgentEvent::AuthRequired {
                methods: auth_methods.to_vec(),
                message: describe_error(&err),
            });
            None
        }
        Err(err) => {
            events.send(AgentEvent::Error(format!(
                "Failed to start session: {}",
                describe_error(&err)
            )));
            None
        }
    }
}

/// Human-readable error text: the message, plus whatever the agent put in `data`.
///
/// The SDK wraps process failures as JSON in `data` with a `data` string inside; we dig that out
/// so the user sees the agent's own stderr instead of a JSON blob.
fn describe_error(err: &Error) -> String {
    let mut text = err.message.clone();
    let mut data = err.data.clone();
    while let Some(value) = data.take() {
        match value {
            serde_json::Value::String(inner) => {
                text = format!("{text}: {inner}");
            }
            serde_json::Value::Object(mut fields) => {
                if let Some(inner) = fields.remove("data") {
                    data = Some(inner);
                } else if let Some(serde_json::Value::String(reason)) = fields.get("reason") {
                    text = format!("{text}: {reason}");
                } else {
                    text = format!("{text}: {}", serde_json::Value::Object(fields));
                }
            }
            other => {
                text = format!("{text}: {other}");
            }
        }
    }
    text
}
