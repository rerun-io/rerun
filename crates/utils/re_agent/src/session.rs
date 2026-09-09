//! The state of one agent session: the connection, the transcript, and whatever the agent
//! is currently waiting on (a permission answer, a login).
//!
//! No egui in here, so a host can drive it from any UI or from a headless test.

use std::collections::VecDeque;

use agent_client_protocol::schema::v1::{
    AuthMethod, AuthMethodId, ContentBlock, PermissionOptionId, PermissionOptionKind,
    RequestPermissionOutcome, RequestPermissionRequest, RequestPermissionResponse,
    SelectedPermissionOutcome, SessionModeId, SessionModeState, SessionUpdate, StopReason,
    TextContent,
};
use agent_client_protocol::{LineDirection, Responder};

use crate::connection::{AgentCommand, AgentConnection, AgentEvent, LaunchConfig};
use crate::transcript::Transcript;

const MAX_LOG_LINES: usize = 500;

/// Where we are in the lifetime of an agent connection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// No agent running.
    #[default]
    Idle,

    /// Agent spawned, waiting for initialize and session start.
    Connecting { status: String },

    /// Session open. Prompts can be sent.
    Ready,

    /// The agent went away. The transcript stays until the next start.
    Disconnected { reason: String },
}

/// A permission request the user has not answered yet. The agent is blocked until they do.
pub struct PendingPermission {
    /// What the agent wants to do, and the options the user can pick from.
    pub request: RequestPermissionRequest,

    /// `None` for requests injected by tests; answering them is a no-op.
    responder: Option<Responder<RequestPermissionResponse>>,
}

impl PendingPermission {
    fn answer(
        self,
        option_id: Option<PermissionOptionId>,
    ) -> Result<(), agent_client_protocol::Error> {
        let outcome = match option_id {
            Some(option_id) => {
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id))
            }
            None => RequestPermissionOutcome::Cancelled,
        };
        match self.responder {
            Some(responder) => responder.respond(RequestPermissionResponse::new(outcome)),
            None => Ok(()),
        }
    }
}

/// The agent refused to open a session until the user logs in.
pub struct AuthPrompt {
    /// Ways to log in, as offered by the agent. Empty if the agent offered none.
    pub methods: Vec<AuthMethod>,

    /// The agent's own explanation of why login is needed.
    pub message: String,
}

/// One line the agent wrote to stderr, or one JSON-RPC line when protocol logging is on.
pub struct LogLine {
    /// Whether the line went to the agent or came from it.
    pub direction: LineDirection,

    /// The raw line, without its trailing newline.
    pub line: String,
}

/// Everything the UI needs to know about the running agent, updated by [`Self::poll_events`].
#[derive(Default)]
pub struct AgentSession {
    connection: Option<AgentConnection>,
    phase: Phase,
    agent_name: Option<String>,
    modes: Option<SessionModeState>,
    auth: Option<AuthPrompt>,
    transcript: Transcript,
    pending_permissions: Vec<PendingPermission>,
    turn_in_progress: bool,
    log: VecDeque<LogLine>,
    auto_approve: bool,

    /// Markdown sent ahead of the first prompt, so the agent knows where it is running.
    preamble: Option<String>,
    prompts_sent: usize,

    /// Set by [`Self::set_mode`] until the agent confirms a mode.
    requested_mode: Option<SessionModeId>,

    /// Prompts typed while a turn was running. Sent one per turn, in order.
    queue: VecDeque<String>,
}

impl AgentSession {
    /// Spawns the agent. `wake` is called whenever new events arrive, so the UI can repaint.
    pub fn start(&mut self, config: LaunchConfig, wake: impl Fn() + Send + Sync + 'static) {
        self.stop();
        *self = Self {
            phase: Phase::Connecting {
                status: format!("Starting {}…", config.command.display()),
            },
            auto_approve: self.auto_approve,
            preamble: config.preamble.clone(),
            ..Default::default()
        };
        self.connection = Some(AgentConnection::spawn(config, wake));
    }

    /// Shuts the agent down. Pending permission requests are answered with "cancelled".
    pub fn stop(&mut self) {
        for pending in self.pending_permissions.drain(..) {
            pending.answer(None).ok();
        }
        self.connection = None;
        if self.turn_in_progress {
            self.transcript.fail_unfinished_tool_calls("Stopped");
        }
        self.turn_in_progress = false;
        self.phase = Phase::Idle;
    }

    /// Answer every permission request with the first "allow" option, without asking.
    /// Meant for unattended runs; the agent gets to do anything it asks for.
    pub fn set_auto_approve(&mut self, auto_approve: bool) {
        self.auto_approve = auto_approve;
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    /// A session is open and the agent is idle.
    pub fn is_ready(&self) -> bool {
        self.phase == Phase::Ready && !self.turn_in_progress
    }

    pub fn turn_in_progress(&self) -> bool {
        self.turn_in_progress
    }

    /// What the agent calls itself, once it has told us.
    pub fn agent_name(&self) -> Option<&str> {
        self.agent_name.as_deref()
    }

    pub fn modes(&self) -> Option<&SessionModeState> {
        self.modes.as_ref()
    }

    pub fn current_mode_id(&self) -> Option<&SessionModeId> {
        self.transcript
            .current_mode
            .as_ref()
            .or_else(|| self.modes.as_ref().map(|modes| &modes.current_mode_id))
    }

    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    pub fn auth(&self) -> Option<&AuthPrompt> {
        self.auth.as_ref()
    }

    pub fn pending_permissions(&self) -> &[PendingPermission] {
        &self.pending_permissions
    }

    pub fn log(&self) -> &VecDeque<LogLine> {
        &self.log
    }

    /// Shows an error inline in the transcript.
    pub fn report_error(&mut self, message: impl Into<String>) {
        self.transcript.push_note(message, true);
    }

    /// Returns `false` if the prompt was not sent because the agent is not ready.
    #[must_use]
    pub fn send_prompt(&mut self, text: impl Into<String>) -> bool {
        let text = text.into();
        if text.trim().is_empty() || self.phase != Phase::Ready {
            return false;
        }
        if self.turn_in_progress {
            self.queue.push_back(text);
            return true;
        }
        let Some(connection) = &self.connection else {
            return false;
        };
        self.transcript.push_user(text.clone());
        self.turn_in_progress = true;

        // The preamble goes only with the first prompt: the agent keeps its own history.
        let mut prompt = Vec::new();
        if self.prompts_sent == 0
            && let Some(preamble) = &self.preamble
        {
            prompt.push(ContentBlock::Text(TextContent::new(preamble.clone())));
        }
        prompt.push(ContentBlock::Text(TextContent::new(text)));
        self.prompts_sent += 1;

        connection.send(AgentCommand::Prompt(prompt));
        true
    }

    /// Prompts waiting for the current turn to finish, in the order they will be sent.
    pub fn queued_prompts(&self) -> &VecDeque<String> {
        &self.queue
    }

    /// Takes a prompt out of the queue, e.g. to put it back into the input.
    pub fn remove_queued(&mut self, index: usize) -> Option<String> {
        self.queue.remove(index)
    }

    /// Empties the queue. Call before [`Self::cancel`], so stopping the agent does not
    /// let the next queued prompt start a new turn.
    pub fn take_queued(&mut self) -> Vec<String> {
        self.queue.drain(..).collect()
    }

    /// Cancels the current turn. Pending permission requests are answered with "cancelled".
    pub fn cancel(&mut self) {
        if let Some(connection) = &self.connection {
            connection.send(AgentCommand::Cancel);
        }
        for pending in self.pending_permissions.drain(..) {
            pending.answer(None).ok();
        }
    }

    pub fn set_mode(&mut self, mode_id: SessionModeId) {
        self.requested_mode = Some(mode_id.clone());
        if let Some(connection) = &self.connection {
            connection.send(AgentCommand::SetMode(mode_id));
        }
    }

    /// The mode last asked for with [`Self::set_mode`], for tests: nothing confirms it
    /// without a connected agent.
    #[cfg(feature = "testing")]
    pub fn requested_mode(&self) -> Option<&SessionModeId> {
        self.requested_mode.as_ref()
    }

    pub fn authenticate(&mut self, method_id: AuthMethodId) {
        if let Some(connection) = &self.connection {
            connection.send(AgentCommand::Authenticate(method_id));
            self.auth = None;
        }
    }

    /// Answers the pending permission request at `index` with the given option,
    /// or cancels it when `option_id` is `None`.
    pub fn answer_permission(&mut self, index: usize, option_id: Option<PermissionOptionId>) {
        if index < self.pending_permissions.len() {
            let pending = self.pending_permissions.remove(index);
            if let Err(err) = pending.answer(option_id) {
                self.report_error(format!("Failed to answer permission request: {err}"));
            }
        }
    }

    /// Applies everything the agent sent since the last call. Call once per frame.
    pub fn poll_events(&mut self) {
        let Some(connection) = &self.connection else {
            return;
        };
        let events: Vec<AgentEvent> = connection.poll_events().collect();
        for event in events {
            self.handle_event(event);
        }
    }

    /// Direct access to the transcript, for tests that script a conversation.
    #[cfg(feature = "testing")]
    pub fn transcript_mut(&mut self) -> &mut Transcript {
        &mut self.transcript
    }

    /// Pretends a turn is running, without an agent. For UI tests.
    #[cfg(feature = "testing")]
    pub fn begin_test_turn(&mut self) {
        self.turn_in_progress = true;
    }

    /// Adds a permission request that no agent is waiting on. For UI tests.
    #[cfg(feature = "testing")]
    pub fn push_test_permission_request(&mut self, request: RequestPermissionRequest) {
        self.transcript
            .apply_tool_call_update(request.tool_call.clone());
        self.pending_permissions.push(PendingPermission {
            request,
            responder: None,
        });
    }

    /// Applies one event as if it came from the agent. Useful for replaying recorded sessions.
    pub fn handle_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Status(status) => {
                if self.phase != Phase::Ready {
                    self.phase = Phase::Connecting { status };
                }
            }
            AgentEvent::Initialized { agent_info, .. } => {
                self.agent_name = agent_info.map(|info| info.title.unwrap_or(info.name));
            }
            AgentEvent::SessionStarted { modes, .. } => {
                self.modes = modes;
                self.auth = None;
                self.phase = Phase::Ready;
            }
            AgentEvent::AuthRequired { methods, message } => {
                self.phase = Phase::Connecting {
                    status: "Waiting for login".to_owned(),
                };
                self.auth = Some(AuthPrompt { methods, message });
            }
            AgentEvent::Update(update) => {
                if let SessionUpdate::CurrentModeUpdate(_) = &update {
                    self.requested_mode = None;
                }
                self.transcript.apply(update);
            }
            AgentEvent::PermissionRequest { request, responder } => {
                self.transcript
                    .apply_tool_call_update(request.tool_call.clone());
                let pending = PendingPermission {
                    request,
                    responder: Some(responder),
                };
                if self.auto_approve
                    && let Some(option) = pending.request.options.iter().find(|option| {
                        matches!(
                            option.kind,
                            PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways
                        )
                    })
                {
                    let option_id = option.option_id.clone();
                    pending.answer(Some(option_id)).ok();
                } else {
                    self.pending_permissions.push(pending);
                }
            }
            AgentEvent::TurnFinished(result) => {
                self.turn_in_progress = false;
                self.transcript.fail_unfinished_tool_calls("Cancelled");
                if !matches!(result, Ok(StopReason::Cancelled))
                    && let Some(next) = self.queue.pop_front()
                {
                    let sent = self.send_prompt(next);
                    re_log::debug_assert!(
                        sent,
                        "the session is ready and queued prompts are not empty"
                    );
                }
                match result {
                    Ok(StopReason::Cancelled) => self.transcript.push_note("Cancelled", false),
                    Ok(StopReason::MaxTokens) => {
                        self.transcript
                            .push_note("Stopped: token limit reached", false);
                    }
                    Ok(StopReason::MaxTurnRequests) => {
                        self.transcript
                            .push_note("Stopped: too many tool calls", false);
                    }
                    Ok(StopReason::Refusal) => {
                        self.transcript
                            .push_note("The agent declined to continue", false);
                    }
                    Ok(_) => {}
                    Err(err) => self.transcript.push_note(err, true),
                }
            }
            AgentEvent::Log { direction, line } => {
                re_log::trace!("agent {direction:?}: {line}");
                if self.log.len() == MAX_LOG_LINES {
                    self.log.pop_front();
                }
                self.log.push_back(LogLine { direction, line });
            }
            AgentEvent::Error(err) => self.transcript.push_note(err, true),
            AgentEvent::Disconnected => {
                self.turn_in_progress = false;
                self.transcript
                    .fail_unfinished_tool_calls("The agent exited before finishing");
                self.phase = Phase::Disconnected {
                    reason: "The agent exited".to_owned(),
                };
                self.connection = None;
                self.pending_permissions.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use agent_client_protocol::schema::v1::{SessionUpdate, ToolCall, ToolCallId, ToolCallStatus};

    use super::*;

    #[test]
    fn unfinished_tool_calls_fail_when_the_turn_ends() {
        let mut session = AgentSession {
            turn_in_progress: true,
            ..Default::default()
        };
        for (id, status) in [
            ("done", ToolCallStatus::Completed),
            ("running", ToolCallStatus::InProgress),
            ("queued", ToolCallStatus::Pending),
        ] {
            session.handle_event(AgentEvent::Update(SessionUpdate::ToolCall(
                ToolCall::new(id, id).status(status),
            )));
        }

        session.handle_event(AgentEvent::TurnFinished(Ok(StopReason::Cancelled)));

        let status = |id: &str| {
            session
                .transcript()
                .tool_call(&ToolCallId::new(id))
                .map(|call| call.status)
        };
        assert_eq!(status("done"), Some(ToolCallStatus::Completed));
        assert_eq!(status("running"), Some(ToolCallStatus::Failed));
        assert_eq!(status("queued"), Some(ToolCallStatus::Failed));
        assert!(!session.turn_in_progress());
    }
}
