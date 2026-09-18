//! The state of one agent session: the connection, the transcript, and whatever the agent
//! is currently waiting on (a permission answer, a login).
//!
//! No egui in here, so a host can drive it from any UI or from a headless test.

use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use agent_client_protocol::schema::v1::{
    AuthMethod, AuthMethodId, ContentBlock, PermissionOption, PermissionOptionId,
    PermissionOptionKind, RequestPermissionOutcome, RequestPermissionRequest,
    RequestPermissionResponse, SelectedPermissionOutcome, SessionModeId, SessionModeState,
    SessionUpdate, StopReason, TextContent, ToolCallStatus, ToolKind,
};
use agent_client_protocol::{LineDirection, Responder};

use crate::connection::{AgentCommand, AgentConnection, AgentEvent, LaunchConfig};
use crate::transcript::{ToolCallState, Transcript, TranscriptItem};
use crate::turn::{TurnOutcome, TurnReport, TurnStart, describe_failed_tool_call};

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
    log: VecDeque<LogLine>,
    auto_approve: bool,

    /// Markdown sent ahead of the first prompt, so the agent knows where it is running.
    preamble: Option<String>,
    prompts_sent: usize,

    /// Set by [`Self::set_mode`] until the agent confirms a mode.
    requested_mode: Option<SessionModeId>,

    /// Prompts typed while a turn was running. Sent one per turn, in order.
    queue: VecDeque<String>,

    /// The turn started by the last prompt, until the agent finishes it.
    current_turn: Option<TurnStart>,

    /// Turns finished since the host last called [`Self::take_finished_turns`].
    finished_turns: Vec<TurnReport>,

    /// Read-only tool calls that stay inside these directories are allowed without asking.
    /// The host put them there for the agent to read, e.g. skills.
    readable_directories: Vec<PathBuf>,

    /// Directories the preamble asked the agent to stay out of. Touching them is reported in
    /// [`TurnReport::off_limits_paths`], never blocked.
    off_limits_directories: Vec<PathBuf>,

    /// Working directory the agent process was started in, which its relative paths are against.
    cwd: PathBuf,
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
            readable_directories: config.additional_directories.clone(),
            off_limits_directories: config.off_limits_directories.clone(),
            cwd: config.cwd.clone(),
            finished_turns: std::mem::take(&mut self.finished_turns),
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
        self.finish_turn(TurnOutcome::Aborted, "Stopped");
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
        self.phase == Phase::Ready && self.current_turn.is_none()
    }

    pub fn turn_in_progress(&self) -> bool {
        self.current_turn.is_some()
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
        if self.current_turn.is_some() {
            self.queue.push_back(text);
            return true;
        }
        if self.connection.is_none() {
            return false;
        }
        self.begin_turn(text.clone());

        // The preamble goes only with the first prompt: the agent keeps its own history.
        let mut prompt = Vec::new();
        if self.prompts_sent == 0
            && let Some(preamble) = &self.preamble
        {
            prompt.push(ContentBlock::Text(TextContent::new(preamble.clone())));
        }
        prompt.push(ContentBlock::Text(TextContent::new(text)));
        self.prompts_sent += 1;

        if let Some(connection) = &self.connection {
            connection.send(AgentCommand::Prompt(prompt));
        }
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

    /// Drains the reports of the turns finished since the last call.
    pub fn take_finished_turns(&mut self) -> Vec<TurnReport> {
        std::mem::take(&mut self.finished_turns)
    }

    fn begin_turn(&mut self, prompt: String) {
        self.transcript.push_user(prompt.clone());
        self.current_turn = Some(TurnStart {
            prompt,
            started: Instant::now(),
            transcript_start: self.transcript.items.len(),
            permissions_requested: 0,
            permissions_rejected: 0,
        });
    }

    /// Closes the current turn, if any, and queues its report.
    ///
    /// Tool calls the agent never finished are failed with `unfinished_reason` first,
    /// so the report counts them.
    fn finish_turn(&mut self, outcome: TurnOutcome, unfinished_reason: &str) {
        let Some(turn) = self.current_turn.take() else {
            return;
        };
        self.transcript
            .fail_unfinished_tool_calls(unfinished_reason);
        let TurnStart {
            prompt,
            started,
            transcript_start,
            permissions_requested,
            permissions_rejected,
        } = turn;

        let items = self
            .transcript
            .items
            .iter()
            .skip(transcript_start)
            .map(|entry| &entry.item);
        let mut tool_calls = 0;
        let mut failed_tool_calls = Vec::new();
        let mut off_limits_paths = Vec::new();
        let mut errors = Vec::new();
        let mut response = String::new();
        for item in items {
            match item {
                TranscriptItem::ToolCall(call) => {
                    tool_calls += 1;
                    if call.status == ToolCallStatus::Failed {
                        failed_tool_calls.push(describe_failed_tool_call(call));
                    }
                    off_limits_paths.extend(paths_within(
                        call,
                        &self.cwd,
                        &self.off_limits_directories,
                    ));
                }
                TranscriptItem::Note { text, is_error } if *is_error => errors.push(text.clone()),
                TranscriptItem::Agent { content, .. } => {
                    let text = content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::Text(text) => Some(text.text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.trim().is_empty() {
                        response = text;
                    }
                }
                TranscriptItem::User { .. } | TranscriptItem::Note { .. } => {}
            }
        }

        if !off_limits_paths.is_empty() {
            re_log::warn!(
                "The agent was asked to stay out of {} but touched {}",
                display_paths(&self.off_limits_directories),
                display_paths(&off_limits_paths)
            );
        }

        self.finished_turns.push(TurnReport {
            agent: self.agent_name.clone(),
            prompt,
            duration: started.elapsed(),
            outcome,
            response,
            tool_calls,
            tokens_used: self.transcript.usage.as_ref().map(|usage| usage.used),
            token_limit: self.transcript.usage.as_ref().map(|usage| usage.size),
            failed_tool_calls,
            off_limits_paths,
            errors,
            permissions_requested,
            permissions_rejected,
        });
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
            let allowed = option_id.as_ref().is_some_and(|option_id| {
                pending
                    .request
                    .options
                    .iter()
                    .any(|option| option.option_id == *option_id && allows(option.kind))
            });
            if !allowed && let Some(turn) = &mut self.current_turn {
                turn.permissions_rejected += 1;
            }
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
        self.current_turn = Some(TurnStart::testing(self.transcript.items.len()));
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
            AgentEvent::SessionStarted {
                modes,
                config_options,
                ..
            } => {
                self.modes = modes;
                self.transcript.config_options = config_options;
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
                let is_harmless_read = self
                    .transcript
                    .tool_call(&pending.request.tool_call.tool_call_id)
                    .is_some_and(|call| is_read_within(call, &self.readable_directories));
                if (self.auto_approve || is_harmless_read)
                    && let Some(option) = allow_option(&pending.request.options)
                {
                    let option_id = option.option_id.clone();
                    pending.answer(Some(option_id)).ok();
                } else {
                    if let Some(turn) = &mut self.current_turn {
                        turn.permissions_requested += 1;
                    }
                    self.pending_permissions.push(pending);
                }
            }
            AgentEvent::TurnFinished(result) => {
                let outcome = match result {
                    Ok(StopReason::Cancelled) => {
                        self.transcript.push_note("Cancelled", false);
                        TurnOutcome::Cancelled
                    }
                    Ok(StopReason::MaxTokens) => {
                        self.transcript
                            .push_note("Stopped: token limit reached", false);
                        TurnOutcome::Truncated
                    }
                    Ok(StopReason::MaxTurnRequests) => {
                        self.transcript
                            .push_note("Stopped: too many tool calls", false);
                        TurnOutcome::Truncated
                    }
                    Ok(StopReason::Refusal) => {
                        self.transcript
                            .push_note("The agent declined to continue", false);
                        TurnOutcome::Refused
                    }
                    Ok(_) => TurnOutcome::Completed,
                    Err(err) => {
                        self.transcript.push_note(err, true);
                        TurnOutcome::Error
                    }
                };
                let cancelled = outcome == TurnOutcome::Cancelled;
                self.finish_turn(outcome, "Cancelled");
                if !cancelled && let Some(next) = self.queue.pop_front() {
                    let sent = self.send_prompt(next);
                    re_log::debug_assert!(
                        sent,
                        "the session is ready and queued prompts are not empty"
                    );
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
                self.finish_turn(TurnOutcome::Aborted, "The agent exited before finishing");
                self.phase = Phase::Disconnected {
                    reason: "The agent exited".to_owned(),
                };
                self.connection = None;
                self.pending_permissions.clear();
            }
        }
    }
}

fn allows(kind: PermissionOptionKind) -> bool {
    matches!(
        kind,
        PermissionOptionKind::AllowOnce | PermissionOptionKind::AllowAlways
    )
}

/// The option that lets the tool call proceed, preferring a one-off allow.
fn allow_option(options: &[PermissionOption]) -> Option<&PermissionOption> {
    options
        .iter()
        .find(|option| option.kind == PermissionOptionKind::AllowOnce)
        .or_else(|| options.iter().find(|option| allows(option.kind)))
}

/// Whether `call` only reads or searches, and every path it touches is inside `directories`.
///
/// Paths come from the call's locations and from every path-shaped string in its raw input.
/// A call without any known path is not considered contained, and neither is one that carries
/// a relative path: that resolves against the agent's working directory, which is the user's
/// project and not ours to hand out.
fn is_read_within(call: &ToolCallState, directories: &[PathBuf]) -> bool {
    if !matches!(call.kind, ToolKind::Read | ToolKind::Search) {
        return false;
    }

    let mut paths = tool_call_paths(call).peekable();
    paths.peek().is_some() && paths.all(|path| is_within(path, directories))
}

/// Every path the call names, from its reported locations and its path-shaped input strings.
///
/// The agent decides what it puts in either, so this is a best effort: a path the agent does not
/// report, or one a shell assembles from a variable or a glob, is not seen here.
fn tool_call_paths(call: &ToolCallState) -> impl Iterator<Item = &Path> {
    let mut strings = Vec::new();
    if let Some(input) = &call.raw_input {
        collect_strings(input, &mut strings);
    }
    let input_paths = strings.into_iter().flat_map(path_like_words);
    let location_paths = call
        .locations
        .iter()
        .map(|location| location.path.as_path());

    std::iter::chain(location_paths, input_paths)
        .collect::<Vec<_>>()
        .into_iter()
}

/// Every string anywhere in `value`, since a tool's arguments nest: an edit carries its path one
/// object down, and an MCP tool decides its own shape.
fn collect_strings<'a>(value: &'a serde_json::Value, out: &mut Vec<&'a str>) {
    match value {
        serde_json::Value::String(string) => out.push(string),
        serde_json::Value::Array(values) => {
            for value in values {
                collect_strings(value, out);
            }
        }
        serde_json::Value::Object(map) => {
            for value in map.values() {
                collect_strings(value, out);
            }
        }
        _ => {}
    }
}

/// `a, b, c`, for naming a set of paths in a log message.
fn display_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// `path` made absolute against `cwd` and lexically normalized.
///
/// Lexical only: nothing is read from disk, so a path that does not exist still resolves, and a
/// symlink is left as written. `..` is popped rather than refused, because a caller reporting
/// what already happened wants to know where the path landed.
fn resolve_against(cwd: &Path, path: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_owned()
    } else {
        cwd.join(path)
    };
    let mut resolved = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
            component => resolved.push(component),
        }
    }
    resolved
}

/// Paths the call touched that lie inside `directories`, whatever kind of tool it is.
///
/// Unlike [`is_read_within`] this covers writes and shell commands too: the point is to notice
/// that a directory was touched at all, not to decide whether it was harmless. For the same
/// reason it resolves a path against `cwd` instead of refusing it for being relative: a report
/// that silently drops `src/lib.rs` says the directory went untouched when it did not.
fn paths_within<'a>(
    call: &'a ToolCallState,
    cwd: &Path,
    directories: &'a [PathBuf],
) -> Vec<PathBuf> {
    if directories.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<PathBuf> = tool_call_paths(call)
        .map(|path| resolve_against(cwd, path))
        .filter(|path| {
            directories
                .iter()
                .any(|directory| path.starts_with(directory))
        })
        .collect();
    hits.sort();
    hits.dedup();
    hits
}

/// Every path-shaped word in `value`.
///
/// A tool's input is not always one path per field: a shell command arrives as a single string,
/// so `cat /home/me/rerun/src/lib.rs` has to be read word by word or the path in it is never
/// seen. A one-word value yields itself, which is the ordinary `file_path` case.
///
/// Shell quoting and punctuation are trimmed off the ends, so a quoted path still matches. What
/// this cannot see is a path the shell assembles, from a variable or a glob.
fn path_like_words(value: &str) -> impl Iterator<Item = &Path> {
    value
        .split_whitespace()
        .map(|word| word.trim_matches(|c| matches!(c, '\'' | '"' | '`' | ',' | ';' | '(' | ')')))
        .filter(|word| looks_like_path(word))
        .map(Path::new)
}

/// Whether `value` names a file or directory, rather than a search pattern, a flag, or a word.
///
/// A bare word is ambiguous, so it is left to the caller's other checks; anything with a
/// separator or a dot-segment is treated as a path and must then pass [`is_within`].
fn looks_like_path(value: &str) -> bool {
    value.contains('/') || value.contains('\\') || value == "." || value == ".."
}

/// Whether `path` is an absolute path inside one of `directories`, without `..` that could
/// escape it.
fn is_within(path: &Path, directories: &[PathBuf]) -> bool {
    let has_parent_dir = path
        .components()
        .any(|component| component == Component::ParentDir);
    path.is_absolute()
        && !has_parent_dir
        && directories
            .iter()
            .any(|directory| path.starts_with(directory))
}

#[cfg(test)]
mod tests {
    use agent_client_protocol::schema::v1::{
        ContentChunk, SessionUpdate, ToolCall, ToolCallContent, ToolCallId, ToolCallLocation,
        ToolCallStatus,
    };

    use super::*;

    #[test]
    fn turn_report() {
        let mut session = AgentSession {
            agent_name: Some("Test Agent".to_owned()),
            ..Default::default()
        };
        session.begin_turn("do the thing".to_owned());
        if let Some(turn) = &mut session.current_turn {
            turn.permissions_requested = 2;
            turn.permissions_rejected = 1;
        }

        let text = |text: &str| ContentBlock::Text(TextContent::new(text.to_owned()));
        for update in [
            SessionUpdate::AgentMessageChunk(ContentChunk::new(text("Looking…"))),
            SessionUpdate::ToolCall(
                ToolCall::new("ok-1", "Read a file").status(ToolCallStatus::Completed),
            ),
            SessionUpdate::ToolCall(
                ToolCall::new("bad-1", "Run tests")
                    .status(ToolCallStatus::Failed)
                    .content(vec![ToolCallContent::from(text(
                        "assertion failed\nmore details",
                    ))]),
            ),
            SessionUpdate::ToolCall(
                ToolCall::new("bad-2", "Fetch")
                    .status(ToolCallStatus::Failed)
                    .raw_output(serde_json::json!("timeout")),
            ),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(text("Done. Two tools failed."))),
        ] {
            session.handle_event(AgentEvent::Update(update));
        }
        session.handle_event(AgentEvent::Error("agent hiccup".to_owned()));
        assert!(session.take_finished_turns().is_empty());

        session.handle_event(AgentEvent::TurnFinished(Ok(StopReason::EndTurn)));
        assert!(!session.turn_in_progress());
        let reports = session.take_finished_turns();
        assert_eq!(reports.len(), 1);
        let report = &reports[0];
        assert_eq!(report.agent.as_deref(), Some("Test Agent"));
        assert_eq!(report.prompt, "do the thing");
        assert_eq!(report.outcome, TurnOutcome::Completed);
        assert_eq!(report.response, "Done. Two tools failed.");
        assert_eq!(report.tool_calls, 3);
        assert_eq!(
            report.failed_tool_calls,
            vec!["Run tests: assertion failed", "Fetch: timeout"]
        );
        assert_eq!(report.errors, vec!["agent hiccup"]);
        assert_eq!(report.permissions_requested, 2);
        assert_eq!(report.permissions_rejected, 1);
        assert!(session.take_finished_turns().is_empty());

        // A turn cut short by the agent going away is still reported.
        session.begin_turn("again".to_owned());
        session.handle_event(AgentEvent::Disconnected);
        let reports = session.take_finished_turns();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].outcome, TurnOutcome::Aborted);
        assert_eq!(reports[0].response, "");
        assert_eq!(reports[0].tool_calls, 0);
    }

    fn call(
        kind: ToolKind,
        locations: &[&str],
        raw_input: Option<serde_json::Value>,
    ) -> ToolCallState {
        ToolCallState {
            id: ToolCallId::new("call"),
            title: String::new(),
            kind,
            status: ToolCallStatus::Pending,
            content: Vec::new(),
            locations: locations
                .iter()
                .map(|path| ToolCallLocation::new(PathBuf::from(path)))
                .collect(),
            raw_input,
            raw_output: None,
        }
    }

    #[test]
    fn read_within_directories() {
        // Only absolute paths are checked, and a Windows path needs a drive letter to be absolute.
        let root = if cfg!(windows) {
            "C:/cache/agent"
        } else {
            "/cache/agent"
        };
        let elsewhere = if cfg!(windows) {
            "C:/etc/passwd"
        } else {
            "/etc/passwd"
        };
        let skill = format!("{root}/skills/SKILL.md");
        let file = format!("{root}/x.md");
        let sibling = format!("{root}-other/x.md");
        let escaping = format!("{root}/../secrets");

        let dirs = vec![PathBuf::from(root)];
        let within = |kind, locations: &[&str], raw_input| {
            is_read_within(&call(kind, locations, raw_input), &dirs)
        };

        assert!(within(ToolKind::Read, &[&skill], None));
        assert!(within(ToolKind::Search, &[root], None));
        assert!(within(
            ToolKind::Read,
            &[],
            Some(serde_json::json!({ "filepath": file, "limit": 10 })),
        ));

        // A pattern is not a path, so it neither contains nor disqualifies the call:
        assert!(within(
            ToolKind::Search,
            &[root],
            Some(serde_json::json!({ "pattern": "fn .*width" })),
        ));

        assert!(!within(ToolKind::Read, &[], None));
        assert!(!within(ToolKind::Read, &[&sibling], None));
        assert!(!within(ToolKind::Read, &[&escaping], None));
        assert!(!within(ToolKind::Edit, &[&skill], None));
        assert!(!within(
            ToolKind::Read,
            &[&file],
            Some(serde_json::json!({ "other": elsewhere })),
        ));

        // A relative path resolves against the agent's working directory, not ours,
        // even when the reported location looks contained:
        assert!(!within(
            ToolKind::Read,
            &[&file],
            Some(serde_json::json!({ "filepath": "../../.ssh/id_rsa" })),
        ));
        assert!(!within(
            ToolKind::Read,
            &[&file],
            Some(serde_json::json!({ "filepath": "src/secrets.rs" })),
        ));
        assert!(!within(
            ToolKind::Read,
            &[],
            Some(serde_json::json!({ "filepath": "." }))
        ));
    }

    #[test]
    fn off_limits_paths_are_reported() {
        let root = if cfg!(windows) { "C:/rerun" } else { "/rerun" };
        let source = format!("{root}/crates/viewer/re_viewer/src/app/mod.rs");
        let sibling = format!("{root}-scratch/notes.md");
        let dirs = vec![PathBuf::from(root)];
        let scratch = Path::new(if cfg!(windows) {
            "C:/scratch"
        } else {
            "/scratch"
        });

        // Any kind of tool counts, not just reads: the point is that the directory was touched.
        assert_eq!(
            paths_within(&call(ToolKind::Execute, &[&source], None), scratch, &dirs),
            vec![PathBuf::from(&source)]
        );
        // Both sources of paths are searched, and a path seen twice is reported once.
        assert_eq!(
            paths_within(
                &call(
                    ToolKind::Read,
                    &[&source],
                    Some(serde_json::json!({ "filepath": source })),
                ),
                scratch,
                &dirs,
            ),
            vec![PathBuf::from(&source)]
        );

        // A shell command names its paths inside one string, which is how an agent that was told
        // to keep out reaches the tree without ever filling in a path field.
        assert_eq!(
            paths_within(
                &call(
                    ToolKind::Execute,
                    &[],
                    Some(serde_json::json!({ "command": format!("grep -rn width '{source}'") })),
                ),
                scratch,
                &dirs,
            ),
            vec![PathBuf::from(&source)]
        );

        // Arguments nest, and a path one level down counts the same.
        assert_eq!(
            paths_within(
                &call(
                    ToolKind::Edit,
                    &[],
                    Some(serde_json::json!({ "edits": [{ "file_path": source }] })),
                ),
                scratch,
                &dirs,
            ),
            vec![PathBuf::from(&source)]
        );

        assert!(paths_within(&call(ToolKind::Read, &[&sibling], None), scratch, &dirs).is_empty());
        // Nothing is off-limits unless the host said so:
        assert!(paths_within(&call(ToolKind::Read, &[&source], None), scratch, &[]).is_empty());
    }

    /// A report that only understood absolute paths said "untouched" for a session whose working
    /// directory was the checkout itself, which is the one case worth catching.
    #[test]
    fn a_relative_path_is_resolved_before_it_is_judged() {
        let root = if cfg!(windows) { "C:/rerun" } else { "/rerun" };
        let dirs = vec![PathBuf::from(root)];
        let inside = Path::new(root).join("crates");

        // Relative to a working directory inside the off-limits tree.
        assert_eq!(
            paths_within(
                &call(
                    ToolKind::Read,
                    &[],
                    Some(serde_json::json!({ "filepath": "top/re_viewer/src/app.rs" })),
                ),
                &inside,
                &dirs,
            ),
            vec![Path::new(root).join("crates/top/re_viewer/src/app.rs")]
        );

        // `..` that climbs back into the tree counts too, rather than being waved through.
        let scratch = Path::new(if cfg!(windows) {
            "C:/rerun-scratch"
        } else {
            "/rerun-scratch"
        });
        assert_eq!(
            paths_within(
                &call(
                    ToolKind::Read,
                    &[],
                    Some(serde_json::json!({ "filepath": "../rerun/Cargo.toml" })),
                ),
                scratch,
                &dirs,
            ),
            vec![Path::new(root).join("Cargo.toml")]
        );

        // A relative path that stays outside is still not reported.
        assert!(
            paths_within(
                &call(
                    ToolKind::Read,
                    &[],
                    Some(serde_json::json!({ "filepath": "notes.md" })),
                ),
                scratch,
                &dirs,
            )
            .is_empty()
        );
    }

    #[test]
    fn unfinished_tool_calls_fail_when_the_turn_ends() {
        let mut session = AgentSession {
            current_turn: Some(TurnStart::testing(0)),
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
