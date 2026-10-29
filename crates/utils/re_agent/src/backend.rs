use std::sync::{Arc, mpsc};

use agent_client_protocol_schema::v1::{
    AgentCapabilities, AuthMethod, AuthMethodId, ContentBlock, CreateElicitationRequest,
    CreateElicitationResponse, Implementation, RequestPermissionRequest, RequestPermissionResponse,
    SessionConfigId, SessionConfigOption, SessionConfigValueId, SessionId, SessionModeId,
    SessionModeState, SessionUpdate, StopReason,
};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::Responder;

/// The UI's end of a running agent: takes [`AgentCommand`]s and produces [`AgentEvent`]s.
///
/// Every backend speaks the ACP vocabulary, whatever it talks to underneath, so that
/// [`crate::AgentSession`] and its [`crate::Transcript`] work the same for all of them.
/// Dropping this asks the agent to shut down.
pub struct AgentConnection {
    command_tx: UnboundedSender<AgentCommand>,
    event_rx: mpsc::Receiver<AgentEvent>,
}

impl AgentConnection {
    /// A connection, and the agent's end of it: the commands to serve, and where to send events.
    ///
    /// `wake` is called after every event so the UI can repaint.
    pub fn new(
        wake: impl Fn() + Send + Sync + 'static,
    ) -> (Self, UnboundedReceiver<AgentCommand>, EventSender) {
        let (command_tx, command_rx) = futures::channel::mpsc::unbounded();
        let (events, event_rx) = EventSender::new(wake);
        let connection = Self {
            command_tx,
            event_rx,
        };
        (connection, command_rx, events)
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

/// Something the UI wants the agent to do.
#[derive(Debug)]
pub enum AgentCommand {
    Prompt(Vec<ContentBlock>),
    Cancel,
    SetMode(SessionModeId),
    SetConfigOption {
        config_id: SessionConfigId,
        value: SessionConfigValueId,
    },
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

        /// The agent's initial configuration, e.g. which model it is about to use.
        config_options: Vec<SessionConfigOption>,
    },

    /// The agent requires authentication to start a session or continue a prompt.
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

    /// Structured user input requested by the agent, separate from tool approval.
    ///
    /// The responder must receive an answer, decline, or cancellation to unblock the agent.
    ElicitationRequest {
        request: CreateElicitationRequest,
        responder: Responder<CreateElicitationResponse>,
    },

    TurnFinished(Result<StopReason, String>),

    Log {
        direction: LineDirection,
        line: String,
    },

    Error(String),

    /// The agent is gone. No more events will follow.
    Disconnected,
}

/// Which way a line between the host and the agent went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineDirection {
    /// Sent to the agent, on its stdin.
    Stdin,

    /// Received from the agent, on its stdout.
    Stdout,

    /// Written by the agent to its stderr.
    Stderr,
}

/// Queues events for the UI thread and wakes it.
#[derive(Clone)]
pub struct EventSender {
    tx: mpsc::Sender<AgentEvent>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl EventSender {
    /// An event sender, and the receiver the UI thread drains.
    ///
    /// `wake` is called after every event so the UI can repaint.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> (Self, mpsc::Receiver<AgentEvent>) {
        // Unbounded on purpose: events are small, the UI drains them every frame, and blocking
        // the agent side would also stall responders the agent is waiting on.
        #[expect(clippy::disallowed_methods)]
        let (tx, rx) = mpsc::channel();
        let sender = Self {
            tx,
            wake: Arc::new(wake),
        };
        (sender, rx)
    }

    pub fn send(&self, event: AgentEvent) {
        if self.tx.send(event).is_ok() {
            (self.wake)();
        }
    }
}
