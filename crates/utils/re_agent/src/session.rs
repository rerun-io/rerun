//! The state of one agent session: the connection, the transcript, and whatever the agent
//! is currently waiting on (a permission answer, structured user input, a login).
//!
//! No egui in here, so a host can drive it from any UI or from a headless test.

use std::collections::{BTreeSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use agent_client_protocol_schema::v1::{
    AuthMethod, AuthMethodId, ContentBlock, CreateElicitationRequest, CreateElicitationResponse,
    ElicitationAction, ElicitationMode, ElicitationScope, ImageContent, PermissionOption,
    PermissionOptionId, PermissionOptionKind, PromptCapabilities, RequestPermissionOutcome,
    RequestPermissionRequest, RequestPermissionResponse, SelectedPermissionOutcome,
    SessionConfigId, SessionConfigValueId, SessionModeId, SessionModeState, SessionUpdate,
    StopReason, TextContent, ToolCallId, ToolCallStatus, ToolKind,
};

use re_async::{AsyncRuntimeHandle, UiWaker};
use web_time::Instant;

use crate::agent_harness::McpStdioServer;
use crate::backend::{AgentCommand, AgentConnection, AgentEvent, LineDirection};
use crate::elicitation::response_note;
use crate::error::McpStartupFailure;
use crate::responder::{AgentStoppedWaiting, Responder};
use crate::settings::AgentLaunch;
use crate::transcript::{ToolCallState, Transcript, TranscriptItem, message_text};
use crate::turn::{TurnOutcome, TurnReport, TurnStart, describe_failed_tool_call};

const MAX_LOG_LINES: usize = 500;

/// Why [`AgentSession::start`] could not start an agent.
#[derive(Debug, thiserror::Error)]
enum StartError {
    #[cfg(not(agent_harness_backend))]
    #[error("Agent subprocesses are not supported on this platform")]
    HarnessUnsupported,

    #[error(transparent)]
    ModelApi(crate::ModelApiError),

    #[error("This host cannot run a model-API agent: it has no async runtime")]
    NoAsyncRuntime,
}

/// An image the user attached to a prompt, already encoded as `mime_type`.
#[derive(Clone, Debug)]
pub struct PromptImage {
    /// The encoded image, as `mime_type` describes it.
    ///
    /// Shared, because the transcript, the queue and the renderer all hold the same image, and
    /// a screenshot runs to megabytes.
    pub bytes: Arc<[u8]>,

    /// The MIME type of `bytes`, e.g. `image/png`.
    pub mime_type: String,

    /// Width and height of the encoded image, in pixels.
    pub resolution: [usize; 2],
}

/// What a single prompt carries: the text as typed, and any images attached to it.
#[derive(Clone, Debug, Default)]
pub struct Prompt {
    /// The prompt as typed.
    pub text: String,

    /// Images attached to it.
    pub images: Vec<PromptImage>,
}

impl From<String> for Prompt {
    fn from(text: String) -> Self {
        Self {
            text,
            images: Vec::new(),
        }
    }
}

impl From<&str> for Prompt {
    fn from(text: &str) -> Self {
        text.to_owned().into()
    }
}

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
    fn answer(self, option_id: Option<PermissionOptionId>) -> Result<(), AgentStoppedWaiting> {
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

/// Identifies a [`PendingElicitation`], for the UI to key its drafts by.
///
/// Distinct from the JSON-RPC request ID, and unique across sessions, so a cached draft cannot
/// attach to a question of another session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElicitationId(u64);

impl ElicitationId {
    fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// A structured question awaiting an explicit user answer, independent of tool permissions.
///
/// Questions are never auto-approved. Dropping an unanswered request responds with cancellation.
pub struct PendingElicitation {
    pub id: ElicitationId,

    /// The agent's question message and requested input schema.
    pub request: CreateElicitationRequest,

    /// Absent for injected UI fixtures or after the response has been consumed.
    responder: Option<Responder<CreateElicitationResponse>>,
}

impl PendingElicitation {
    fn answer(mut self, response: CreateElicitationResponse) -> Result<(), AgentStoppedWaiting> {
        // Consume the responder before sending so that Drop cannot send a second response on error.
        match self.responder.take() {
            Some(responder) => responder.respond(response),
            None => Ok(()),
        }
    }
}

impl Drop for PendingElicitation {
    fn drop(&mut self) {
        if let Some(responder) = self.responder.take()
            && let Err(err) =
                responder.respond(CreateElicitationResponse::new(ElicitationAction::Cancel))
        {
            re_log::debug!("Failed to cancel elicitation request: {err}");
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

/// What the host started the agent with.
#[derive(Default)]
struct LaunchContext {
    /// MCP definitions supplied by the host when this session was started.
    mcp_servers: Vec<McpStdioServer>,

    /// Markdown sent ahead of the first prompt, so the agent knows where it is running.
    preamble: Option<String>,

    /// Read-only tool calls that stay inside these directories are allowed without asking.
    /// The host put them there for the agent to read, e.g. skills.
    readable_directories: Vec<PathBuf>,

    /// Directories the preamble asked the agent to stay out of. Touching them is reported in
    /// [`TurnReport::off_limits_paths`], never blocked.
    off_limits_directories: Vec<PathBuf>,

    /// Working directory the agent process was started in, which its relative paths are against.
    cwd: PathBuf,
}

/// What the agent reported about itself, and what was asked of it.
#[derive(Default)]
struct AgentInfo {
    name: Option<String>,
    modes: Option<SessionModeState>,

    /// Set by [`AgentSession::set_mode`] until the agent confirms a mode.
    requested_mode: Option<SessionModeId>,

    auth: Option<AuthPrompt>,

    /// What the agent accepts in a prompt, as it reported on initialization. An agent that takes
    /// no images gets the paths of files instead — see [`AgentSession::send_prompt`].
    prompt_capabilities: PromptCapabilities,
}

/// Questions from the agent that wait on the user.
#[derive(Default)]
struct PendingRequests {
    permissions: Vec<PendingPermission>,
    elicitations: Vec<PendingElicitation>,

    /// Reject questions racing with cancellation until another prompt or session starts.
    cancel_requested: bool,
}

/// Prompts, and the turns they start.
#[derive(Default)]
struct Turns {
    /// Prompts typed while a turn was running. Sent one per turn, in order.
    queue: VecDeque<Prompt>,

    prompts_sent: usize,

    /// The turn started by the last prompt, until the agent finishes it.
    current: Option<TurnStart>,

    /// Turns finished since the host last called [`AgentSession::take_finished_turns`].
    finished: Vec<TurnReport>,
}

/// Images written out for an agent that cannot take them inline.
#[cfg(agent_harness_backend)]
#[derive(Default)]
struct ImageFiles {
    /// Deleting it with the session is the point: the files are only meant to outlive the prompt
    /// that names them.
    dir: Option<tempfile::TempDir>,

    /// Images written to [`Self::dir`] so far, so their file names stay unique.
    written: usize,
}

/// Everything the UI needs to know about the running agent, updated by [`Self::poll_events`].
#[derive(Default)]
pub struct AgentSession {
    connection: Option<AgentConnection>,
    phase: Phase,
    transcript: Transcript,
    log: VecDeque<LogLine>,
    auto_approve: bool,
    launch: LaunchContext,
    agent: AgentInfo,
    requests: PendingRequests,
    turns: Turns,

    #[cfg(agent_harness_backend)]
    images: ImageFiles,
}

impl Drop for AgentSession {
    fn drop(&mut self) {
        // Pending elicitations cancel themselves on drop, and the connection shuts the agent down.
        for pending in self.requests.permissions.drain(..) {
            pending.answer(None).ok();
        }
    }
}

impl AgentSession {
    /// Starts whichever kind of agent `launch` describes.
    ///
    /// A model-API agent runs on `async_runtime`, and fails to start without one.
    /// `waker` is woken whenever new events arrive, so that the UI can
    /// call [`Self::poll_events`].
    ///
    /// Dropping the session shuts the agent down and cancels its pending requests.
    pub fn start(
        launch: AgentLaunch,
        async_runtime: Option<&AsyncRuntimeHandle>,
        waker: UiWaker,
    ) -> Self {
        let mut session = Self::default();
        let connection = match launch {
            AgentLaunch::AgentHarness(config) => {
                session.phase = Phase::Connecting {
                    status: format!("Starting {}…", config.command.display()),
                };
                session.launch.mcp_servers = config.mcp_servers.clone();
                session.launch.preamble = config.preamble.clone();
                session.launch.readable_directories = config.additional_directories.clone();
                session.launch.off_limits_directories = config.off_limits_directories.clone();
                session.launch.cwd = config.cwd.clone();
                cfg_select! {
                    agent_harness_backend => Ok(crate::agent_harness::spawn(config, waker)),
                    _ => {{
                        drop((config, waker));
                        Err(StartError::HarnessUnsupported)
                    }}
                }
            }
            AgentLaunch::ModelApi(config) => {
                session.phase = Phase::Connecting {
                    status: format!("Connecting to {}…", config.model),
                };
                match async_runtime {
                    Some(async_runtime) => {
                        crate::model_api_backend::spawn(config, async_runtime, waker)
                            .map_err(StartError::ModelApi)
                    }
                    None => Err(StartError::NoAsyncRuntime),
                }
            }
        };
        match connection {
            Ok(connection) => session.connection = Some(connection),
            Err(err) => {
                let reason = err.to_string();
                session.report_error(reason.clone());
                session.phase = Phase::Disconnected { reason };
            }
        }
        session
    }

    /// Ends the session and returns every turn report that the host has not taken yet,
    /// including the running turn, reported as aborted.
    pub fn into_finished_turns(mut self) -> Vec<TurnReport> {
        self.finish_turn(TurnOutcome::Aborted, "Stopped");
        std::mem::take(&mut self.turns.finished)
    }

    /// Answer every permission request with the first "allow" option, without asking.
    /// Meant for unattended runs; the agent gets to do anything it asks for.
    pub fn set_auto_approve(&mut self, auto_approve: bool) {
        self.auto_approve = auto_approve;
    }

    /// See [`Self::set_auto_approve`].
    pub fn auto_approve(&self) -> bool {
        self.auto_approve
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    /// A session is open and the agent is idle.
    pub fn is_ready(&self) -> bool {
        self.phase == Phase::Ready && self.turns.current.is_none()
    }

    pub fn turn_in_progress(&self) -> bool {
        self.turns.current.is_some()
    }

    /// Whether the user has sent a prompt and no turn is running.
    ///
    /// A cancelled or failed turn counts as finished.
    pub fn has_finished_turn(&self) -> bool {
        !self.turn_in_progress()
            && self
                .transcript
                .items
                .iter()
                .any(|entry| matches!(entry.item, TranscriptItem::User(_)))
    }

    /// What the agent calls itself, once it has told us.
    pub fn agent_name(&self) -> Option<&str> {
        self.agent.name.as_deref()
    }

    pub fn modes(&self) -> Option<&SessionModeState> {
        self.agent.modes.as_ref()
    }

    pub fn current_mode_id(&self) -> Option<&SessionModeId> {
        self.transcript.current_mode.as_ref().or_else(|| {
            self.agent
                .modes
                .as_ref()
                .map(|modes| &modes.current_mode_id)
        })
    }

    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    pub fn auth(&self) -> Option<&AuthPrompt> {
        self.agent.auth.as_ref()
    }

    pub fn pending_permissions(&self) -> &[PendingPermission] {
        &self.requests.permissions
    }

    /// Questions awaiting a response, in arrival order.
    ///
    /// Polling removes requests withdrawn by the agent.
    pub fn pending_elicitations(&self) -> &[PendingElicitation] {
        &self.requests.elicitations
    }

    /// The agent is blocked on the user: a login, a permission request, or a question.
    pub fn is_waiting_on_user(&self) -> bool {
        self.agent.auth.is_some()
            || !self.requests.permissions.is_empty()
            || !self.requests.elicitations.is_empty()
    }

    /// The tool call is blocked on the user: a permission request or a question names it.
    ///
    /// A login, or a question that names no tool call, could belong to any running call, so
    /// those count for all of them.
    pub fn is_tool_call_waiting_on_user(&self, id: &ToolCallId) -> bool {
        let permission = self
            .requests
            .permissions
            .iter()
            .any(|pending| pending.request.tool_call.tool_call_id == *id);
        let question = self.requests.elicitations.iter().any(|pending| {
            elicitation_tool_call(&pending.request).is_none_or(|tool_call| tool_call == id)
        });
        self.agent.auth.is_some() || permission || question
    }

    pub fn log(&self) -> &VecDeque<LogLine> {
        &self.log
    }

    /// Shows an error inline in the transcript.
    pub fn report_error(&mut self, message: impl Into<String>) {
        self.transcript.push_note(message, true);
    }

    /// Whether the agent takes images in a prompt, as it reported on initialization.
    ///
    /// It is not a precondition of [`Self::send_prompt`]: an agent that says no gets
    /// file paths instead. A UI can still use it to say which of the two will happen.
    pub fn accepts_images(&self) -> bool {
        self.agent.prompt_capabilities.image
    }

    /// Sends `prompt`, or queues it if a turn is running.
    ///
    /// An agent that reported no image support gets each image written to a temporary directory
    /// and named in the prompt text instead, because a stray `ContentBlock::Image` is a protocol
    /// violation, while a path costs the agent one read.
    ///
    /// Returns `false` if the prompt was not sent because the agent is not ready.
    #[must_use]
    pub fn send_prompt(&mut self, prompt: Prompt) -> bool {
        if (prompt.text.trim().is_empty() && prompt.images.is_empty()) || self.phase != Phase::Ready
        {
            return false;
        }
        if self.turns.current.is_some() {
            self.turns.queue.push_back(prompt);
            return true;
        }
        if self.connection.is_none() {
            return false;
        }

        let (prompt_text, image_blocks, unsaved) = self.attach_images(&prompt.text, &prompt.images);
        // The transcript shows the prompt as typed, not `prompt_text`: the file paths appended
        // for an agent that cannot take images inline are plumbing, and the images themselves
        // are in the transcript entry.
        self.begin_turn(prompt);
        self.requests.cancel_requested = false;
        if 0 < unsaved {
            // The image is in the transcript either way, so without this the user would think
            // the agent got something it never did.
            let what = if unsaved == 1 {
                "An image".to_owned()
            } else {
                format!("{unsaved} images")
            };
            self.transcript.push_note(
                format!("{what} could not be saved for the agent to read"),
                true,
            );
        }

        // The preamble goes only with the first prompt: the agent keeps its own history.
        let mut blocks = Vec::new();
        if self.turns.prompts_sent == 0
            && let Some(preamble) = &self.launch.preamble
        {
            blocks.push(ContentBlock::Text(TextContent::new(preamble.clone())));
        }
        if !prompt_text.is_empty() {
            blocks.push(ContentBlock::Text(TextContent::new(prompt_text)));
        }
        blocks.extend(image_blocks);
        self.turns.prompts_sent += 1;

        if let Some(connection) = &self.connection {
            connection.send(AgentCommand::Prompt(blocks));
        }
        true
    }

    /// Turns `images` into what the prompt carries: the text to send, the blocks to append, and
    /// how many images never reached the agent because they could not be written to disk.
    #[cfg_attr(
        not(agent_harness_backend),
        expect(
            clippy::needless_pass_by_ref_mut,
            reason = "only an agent harness takes images as files, which needs `&mut self`"
        )
    )]
    fn attach_images(
        &mut self,
        text: &str,
        images: &[PromptImage],
    ) -> (String, Vec<ContentBlock>, usize) {
        use std::fmt::Write as _;

        let mut prompt_text = text.to_owned();
        let mut blocks = Vec::new();
        let mut for_files = Vec::new();

        for image in images {
            if self.agent.prompt_capabilities.image {
                let data = base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &image.bytes,
                );
                blocks.push(ContentBlock::Image(ImageContent::new(
                    data,
                    image.mime_type.clone(),
                )));
            } else {
                for_files.push(image);
            }
        }

        let (paths, unsaved) = cfg_select! {
            agent_harness_backend => {{
                let mut paths = Vec::new();
                let mut unsaved = 0;
                for image in for_files {
                    match self.write_image(image) {
                        Some(path) => paths.push(path),
                        None => unsaved += 1,
                    }
                }
                (paths, unsaved)
            }}
            // The web has no file system to hand images over through.
            _ => (Vec::<PathBuf>::new(), for_files.len()),
        };

        if !paths.is_empty() {
            prompt_text
                .push_str("\n\nThe user pasted the following image(s). Read them if they matter:");
            for path in paths {
                _ = write!(prompt_text, "\n- `{}`", path.display());
            }
        }

        (prompt_text, blocks, unsaved)
    }

    /// Writes `image` into the session's image directory, for an agent that cannot take it inline.
    ///
    /// The directory is added to the readable ones, so reading back what the user just pasted
    /// does not raise a permission prompt.
    #[cfg(agent_harness_backend)]
    fn write_image(&mut self, image: &PromptImage) -> Option<PathBuf> {
        if self.images.dir.is_none() {
            match tempfile::Builder::new()
                .prefix("rerun-agent-images")
                .tempdir()
            {
                Ok(dir) => {
                    self.launch
                        .readable_directories
                        .push(dir.path().to_path_buf());
                    self.images.dir = Some(dir);
                }
                Err(err) => {
                    re_log::warn!("Could not create a directory for pasted images: {err}");
                    return None;
                }
            }
        }
        let dir = self.images.dir.as_ref()?.path();

        self.images.written += 1;
        let extension = image.mime_type.rsplit('/').next().unwrap_or("png");
        let path = dir.join(format!("pasted-{}.{extension}", self.images.written));
        match std::fs::write(&path, &image.bytes) {
            Ok(()) => Some(path),
            Err(err) => {
                re_log::warn!(?path, "Could not save a pasted image: {err}");
                None
            }
        }
    }

    /// Prompts waiting for the current turn to finish, in the order they will be sent.
    pub fn queued_prompts(&self) -> &VecDeque<Prompt> {
        &self.turns.queue
    }

    /// Takes a prompt out of the queue, e.g. to put it back into the input.
    pub fn remove_queued(&mut self, index: usize) -> Option<Prompt> {
        self.turns.queue.remove(index)
    }

    /// Empties the queue. Call before [`Self::cancel`], so stopping the agent does not
    /// let the next queued prompt start a new turn.
    pub fn take_queued(&mut self) -> Vec<Prompt> {
        self.turns.queue.drain(..).collect()
    }

    /// Drains the reports of the turns finished since the last call.
    pub fn take_finished_turns(&mut self) -> Vec<TurnReport> {
        std::mem::take(&mut self.turns.finished)
    }

    fn begin_turn(&mut self, prompt: Prompt) {
        let text = prompt.text.clone();
        self.transcript.push_user(prompt);
        self.turns.current = Some(TurnStart {
            prompt: text,
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
        let Some(turn) = self.turns.current.take() else {
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
        let mut off_limits_paths = BTreeSet::new();
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
                        &self.launch.cwd,
                        &self.launch.off_limits_directories,
                    ));
                }
                TranscriptItem::Note { text, is_error } if *is_error => errors.push(text.clone()),
                TranscriptItem::Agent { content, .. } => {
                    let text = message_text(content);
                    if !text.trim().is_empty() {
                        response = text;
                    }
                }
                TranscriptItem::User { .. }
                | TranscriptItem::McpStartupFailure(_)
                | TranscriptItem::Note { .. } => {}
            }
        }
        let off_limits_paths: Vec<PathBuf> = off_limits_paths.into_iter().collect();

        if !off_limits_paths.is_empty() {
            re_log::warn!(
                "The agent was asked to stay out of {} but touched {}",
                display_paths(&self.launch.off_limits_directories),
                display_paths(&off_limits_paths)
            );
        }

        self.turns.finished.push(TurnReport {
            agent: self.agent.name.clone(),
            model: self.transcript.current_model().map(str::to_owned),
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

    /// Cancels the current turn, including pending permission and elicitation requests.
    pub fn cancel(&mut self) {
        self.requests.cancel_requested = true;
        if let Some(connection) = &self.connection {
            connection.send(AgentCommand::Cancel);
        }
        for pending in self.requests.permissions.drain(..) {
            pending.answer(None).ok();
        }
        self.requests.elicitations.clear();
    }

    pub fn set_mode(&mut self, mode_id: SessionModeId) {
        self.agent.requested_mode = Some(mode_id.clone());
        if let Some(connection) = &self.connection {
            connection.send(AgentCommand::SetMode(mode_id));
        }
    }

    /// Asks the agent to switch the session to another model, e.g. from [`Transcript::model_selector`].
    pub fn set_config_option(&self, config_id: SessionConfigId, value: SessionConfigValueId) {
        if let Some(connection) = &self.connection {
            connection.send(AgentCommand::SetConfigOption { config_id, value });
        }
    }

    /// The mode last asked for with [`Self::set_mode`], for tests: nothing confirms it
    /// without a connected agent.
    #[cfg(feature = "testing")]
    pub fn requested_mode(&self) -> Option<&SessionModeId> {
        self.agent.requested_mode.as_ref()
    }

    pub fn authenticate(&mut self, method_id: AuthMethodId) {
        if let Some(connection) = &self.connection {
            connection.send(AgentCommand::Authenticate(method_id));
            self.agent.auth = None;
        }
    }

    /// Answers the pending permission request at `index` with the given option,
    /// or cancels it when `option_id` is `None`.
    pub fn answer_permission(&mut self, index: usize, option_id: Option<PermissionOptionId>) {
        if index < self.requests.permissions.len() {
            let pending = self.requests.permissions.remove(index);
            let allowed = option_id.as_ref().is_some_and(|option_id| {
                pending
                    .request
                    .options
                    .iter()
                    .any(|option| option.option_id == *option_id && allows(option.kind))
            });
            if !allowed && let Some(turn) = &mut self.turns.current {
                turn.permissions_rejected += 1;
            }
            if let Err(err) = pending.answer(option_id) {
                self.report_error(format!("Failed to answer permission request: {err}"));
            }
        }
    }

    /// Responds to a pending question and records its answer in the transcript.
    ///
    /// The caller must validate accepted content against the request's schema.
    /// Unknown or already-answered IDs are ignored.
    pub fn answer_elicitation(&mut self, id: ElicitationId, response: CreateElicitationResponse) {
        if let Some(index) = self
            .requests
            .elicitations
            .iter()
            .position(|pending| pending.id == id)
        {
            let pending = self.requests.elicitations.remove(index);
            let note = response_note(&pending.request, &response);
            match pending.answer(response) {
                Ok(()) => self.transcript.push_note(note, false),
                Err(err) => self.report_error(format!("Failed to answer question: {err}")),
            }
        }
    }

    fn push_elicitation(
        &mut self,
        request: CreateElicitationRequest,
        responder: Option<Responder<CreateElicitationResponse>>,
    ) {
        let pending = PendingElicitation {
            id: ElicitationId::next(),
            request,
            responder,
        };
        if self.requests.cancel_requested {
            return;
        }
        if matches!(pending.request.mode, ElicitationMode::Form(_)) {
            self.requests.elicitations.push(pending);
        } else {
            // Even unadvertised modes need a terminal response; leaving them pending blocks the agent.
            if let Err(err) =
                pending.answer(CreateElicitationResponse::new(ElicitationAction::Decline))
            {
                re_log::debug!("Failed to decline unsupported elicitation request: {err}");
            }
        }
    }

    /// Applies everything the agent sent since the last call. Call once per frame.
    pub fn poll_events(&mut self) {
        self.requests.permissions.retain(|pending| {
            pending
                .responder
                .as_ref()
                .is_none_or(|responder| !responder.is_cancelled())
        });
        self.requests.elicitations.retain(|pending| {
            pending
                .responder
                .as_ref()
                .is_none_or(|responder| !responder.is_cancelled())
        });
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
        self.requests.cancel_requested = false;
        self.turns.current = Some(TurnStart::testing(self.transcript.items.len()));
    }

    /// Adds a permission request that no agent is waiting on. For UI tests.
    #[cfg(feature = "testing")]
    pub fn push_test_permission_request(&mut self, request: RequestPermissionRequest) {
        self.transcript
            .apply_tool_call_update(request.tool_call.clone());
        self.requests.permissions.push(PendingPermission {
            request,
            responder: None,
        });
    }

    /// Injects a question with no waiting agent.
    ///
    /// Submitted answers are still recorded in the transcript.
    #[cfg(feature = "testing")]
    pub fn push_test_elicitation_request(&mut self, request: CreateElicitationRequest) {
        self.push_elicitation(request, None);
    }

    /// Applies one event as if it came from the agent. Useful for replaying recorded sessions.
    pub fn handle_event(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::Status(status) => {
                if self.phase != Phase::Ready {
                    self.phase = Phase::Connecting { status };
                }
            }
            AgentEvent::Initialized {
                agent_info,
                capabilities,
                ..
            } => {
                self.agent.name = agent_info.map(|info| info.title.unwrap_or(info.name));
                self.agent.prompt_capabilities = capabilities.prompt_capabilities;
            }
            AgentEvent::SessionStarted {
                modes,
                config_options,
                ..
            } => {
                self.agent.modes = modes;
                self.transcript.config_options = config_options;
                self.agent.auth = None;
                self.phase = Phase::Ready;
            }
            AgentEvent::AuthRequired { methods, message } => {
                self.phase = Phase::Connecting {
                    status: "Waiting for login".to_owned(),
                };
                self.agent.auth = Some(AuthPrompt { methods, message });
            }
            AgentEvent::Update(update) => {
                if let SessionUpdate::CurrentModeUpdate(_) = &update {
                    self.agent.requested_mode = None;
                }
                if let Some(failure) =
                    McpStartupFailure::from_codex_update(&update, &self.launch.mcp_servers)
                {
                    if let Some(note) = failure.replaced_by_agent_config_note() {
                        self.transcript.push_note(note, false);
                    } else {
                        self.transcript.push_mcp_startup_failure(failure);
                    }
                } else {
                    self.transcript.apply(update);
                }
            }
            AgentEvent::PermissionRequest { request, responder } => {
                self.transcript
                    .apply_tool_call_update(request.tool_call.clone());
                let pending = PendingPermission {
                    request,
                    responder: Some(responder),
                };
                if self.requests.cancel_requested {
                    pending.answer(None).ok();
                    return;
                }
                let is_harmless_read = self
                    .transcript
                    .tool_call(&pending.request.tool_call.tool_call_id)
                    .is_some_and(|call| is_read_within(call, &self.launch.readable_directories));
                if (self.auto_approve || is_harmless_read)
                    && let Some(option) = allow_option(&pending.request.options)
                {
                    let option_id = option.option_id.clone();
                    pending.answer(Some(option_id)).ok();
                } else {
                    if let Some(turn) = &mut self.turns.current {
                        turn.permissions_requested += 1;
                    }
                    self.requests.permissions.push(pending);
                }
            }
            AgentEvent::ElicitationRequest { request, responder } => {
                self.push_elicitation(request, Some(responder));
            }
            AgentEvent::TurnFinished(result) => {
                self.requests.elicitations.clear();
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
                if !cancelled && let Some(next) = self.turns.queue.pop_front() {
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
                self.requests.permissions.clear();
                self.requests.elicitations.clear();
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

/// The tool call a question is about, if the agent says.
fn elicitation_tool_call(request: &CreateElicitationRequest) -> Option<&ToolCallId> {
    let scope = match &request.mode {
        ElicitationMode::Form(form) => &form.scope,
        ElicitationMode::Url(url) => &url.scope,
        _ => return None,
    };
    match scope {
        ElicitationScope::Session(session) => session.tool_call_id.as_ref(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use agent_client_protocol_schema::v1::{
        ContentChunk, SessionUpdate, ToolCall, ToolCallContent, ToolCallId, ToolCallLocation,
        ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields,
    };

    use super::*;

    #[cfg(agent_harness_backend)]
    #[test]
    fn question_answers_and_cancellation_reach_the_agent() {
        use agent_client_protocol::{Agent, Channel, Client, ConnectionTo};
        use agent_client_protocol_schema::v1::{ElicitationAcceptAction, ElicitationContentValue};

        let (client_channel, agent_channel) = Channel::duplex();
        let (done_tx, done_rx) = futures::channel::oneshot::channel();
        futures::executor::block_on(async {
            let client = Client
                .builder()
                .on_receive_request(
                    async |request: CreateElicitationRequest, acp_responder, cx| {
                        let (responder, task) = crate::agent_harness::forward_answer(acp_responder);
                        cx.spawn(task)?;
                        let cancel = request.message == "Cancel this question";
                        let mut session = AgentSession::default();
                        session.handle_event(AgentEvent::ElicitationRequest { request, responder });
                        if cancel {
                            drop(session);
                        } else {
                            let id = session.pending_elicitations()[0].id;
                            session.answer_elicitation(
                                id,
                                CreateElicitationResponse::new(
                                    ElicitationAcceptAction::new().content(
                                        std::collections::BTreeMap::from([(
                                            "environment".to_owned(),
                                            ElicitationContentValue::String("existing".to_owned()),
                                        )]),
                                    ),
                                ),
                            );
                        }
                        Ok(())
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .connect_with(client_channel, async move |_cx| {
                    done_rx.await.expect("agent finished");
                    Ok::<_, agent_client_protocol::Error>(())
                });
            let agent = Agent.builder().connect_with(
                agent_channel,
                async move |cx: ConnectionTo<Client>| {
                    let answer = cx.send_request(form_question()).block_task().await?;
                    assert_eq!(
                        serde_json::to_value(answer).expect("response"),
                        serde_json::json!({
                            "action": "accept", "content": { "environment": "existing" }
                        })
                    );
                    let mut request = form_question();
                    request.message = "Cancel this question".to_owned();
                    let answer = cx.send_request(request).block_task().await?;
                    assert_eq!(
                        serde_json::to_value(answer).expect("response"),
                        serde_json::json!({ "action": "cancel" })
                    );
                    done_tx.send(()).expect("client waiting");
                    Ok::<_, agent_client_protocol::Error>(())
                },
            );
            let (client, agent) = futures::join!(client, agent);
            client.expect("client connection");
            agent.expect("agent connection");
        });
    }

    fn form_question() -> CreateElicitationRequest {
        serde_json::from_value(serde_json::json!({
            "mode": "form",
            "sessionId": "test-session",
            "message": "Which environment?",
            "requestedSchema": {
                "type": "object",
                "properties": { "environment": { "type": "string", "enum": ["existing", "temporary"] } }
            }
        })).expect("valid question")
    }

    #[test]
    fn a_question_waits_on_its_tool_call_or_on_all_of_them() {
        let asking = ToolCallId::new("asking");
        let other = ToolCallId::new("other");

        let mut session = AgentSession::default();
        assert!(!session.is_tool_call_waiting_on_user(&asking));

        let mut scoped = form_question();
        if let ElicitationMode::Form(form) = &mut scoped.mode
            && let ElicitationScope::Session(scope) = &mut form.scope
        {
            scope.tool_call_id = Some(asking.clone());
        }
        session.push_elicitation(scoped, None);
        assert!(session.is_tool_call_waiting_on_user(&asking));
        assert!(!session.is_tool_call_waiting_on_user(&other));

        session.push_elicitation(form_question(), None);
        assert!(
            session.is_tool_call_waiting_on_user(&other),
            "unscoped questions count for all"
        );
    }

    #[test]
    fn questions_require_an_answer_even_with_auto_approve() {
        let mut session = AgentSession::default();
        session.set_auto_approve(true);
        session.push_elicitation(form_question(), None);
        assert_eq!(session.pending_elicitations().len(), 1);
        let id = session.pending_elicitations()[0].id;
        session.answer_elicitation(
            id,
            CreateElicitationResponse::new(ElicitationAction::Decline),
        );
        assert!(session.pending_elicitations().is_empty());
        assert!(session.transcript().to_plain_text().contains("Skipped"));
    }

    #[test]
    fn cancelling_discards_pending_and_late_questions() {
        let mut session = AgentSession::default();
        session.push_elicitation(form_question(), None);
        session.cancel();
        assert!(session.pending_elicitations().is_empty());
        session.push_elicitation(form_question(), None);
        assert!(session.pending_elicitations().is_empty());
    }

    fn permission_request() -> RequestPermissionRequest {
        RequestPermissionRequest::new(
            "test-session",
            ToolCallUpdate::new("call", ToolCallUpdateFields::new()),
            vec![PermissionOption::new(
                "allow",
                "Allow",
                PermissionOptionKind::AllowOnce,
            )],
        )
    }

    #[test]
    fn cancelling_rejects_late_permission_requests() {
        let mut session = AgentSession::default();
        session.cancel();
        let (responder, mut answer) = Responder::channel();
        session.handle_event(AgentEvent::PermissionRequest {
            request: permission_request(),
            responder,
        });
        assert!(session.pending_permissions().is_empty());
        let answer = answer.try_recv().expect("answered").expect("answered");
        assert!(matches!(
            answer.outcome,
            RequestPermissionOutcome::Cancelled
        ));
    }

    #[test]
    fn dropping_the_session_cancels_pending_permissions() {
        let mut session = AgentSession::default();
        let (responder, mut answer) = Responder::channel();
        session.handle_event(AgentEvent::PermissionRequest {
            request: permission_request(),
            responder,
        });
        drop(session);
        let answer = answer.try_recv().expect("answered").expect("answered");
        assert!(matches!(
            answer.outcome,
            RequestPermissionOutcome::Cancelled
        ));
    }

    #[test]
    fn ending_a_session_reports_its_running_turn_as_aborted() {
        let mut session = AgentSession::default();
        session.begin_turn("do the thing".into());
        let turns = session.into_finished_turns();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].outcome, TurnOutcome::Aborted);
    }

    #[test]
    fn permission_requests_the_agent_gave_up_on_are_dropped() {
        let mut session = AgentSession::default();
        let (responder, answer) = Responder::channel();
        session.handle_event(AgentEvent::PermissionRequest {
            request: permission_request(),
            responder,
        });
        assert_eq!(session.pending_permissions().len(), 1);
        drop(answer);
        session.poll_events();
        assert!(session.pending_permissions().is_empty());
    }

    #[test]
    fn questions_are_cleared_when_the_turn_ends() {
        for end in ["finish", "disconnect"] {
            let mut session = AgentSession::default();
            session.push_elicitation(form_question(), None);
            match end {
                "finish" => session.handle_event(AgentEvent::TurnFinished(Ok(StopReason::EndTurn))),
                _ => session.handle_event(AgentEvent::Disconnected),
            }
            assert!(session.pending_elicitations().is_empty());
        }
    }

    #[test]
    fn turn_report() {
        let mut session = AgentSession::default();
        session.agent.name = Some("Test Agent".to_owned());
        session.begin_turn("do the thing".into());
        if let Some(turn) = &mut session.turns.current {
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
        session.begin_turn("again".into());
        session.handle_event(AgentEvent::Disconnected);
        let reports = session.take_finished_turns();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].outcome, TurnOutcome::Aborted);
        assert_eq!(reports[0].response, "");
        assert_eq!(reports[0].tool_calls, 0);
    }

    #[test]
    fn codex_mcp_startup_failures_keep_the_requested_definition_out_of_tool_reports() {
        let requested = McpStdioServer {
            name: "rerun".to_owned(),
            command: PathBuf::from("/Applications/Rerun.app/Contents/MacOS/rerun"),
            args: vec!["viewer-mcp".to_owned()],
        };
        let mut session = AgentSession::default();
        session.launch.mcp_servers = vec![requested.clone()];
        session.begin_turn("show the recording".into());

        let error = "[codex-acp forwarded startup error] MCP server rerun failed to start";
        session.handle_event(AgentEvent::Update(SessionUpdate::ToolCall(
            ToolCall::new("mcp_startup.rerun", "mcp__rerun__startup")
                .status(ToolCallStatus::Failed)
                .content(vec![ToolCallContent::from(ContentBlock::Text(
                    TextContent::new(error),
                ))]),
        )));

        let TranscriptItem::McpStartupFailure(failure) = &session.transcript.items[1].item else {
            panic!("expected a dedicated MCP startup failure");
        };
        assert_eq!(failure.server_name, "rerun");
        assert_eq!(failure.requested_command.as_ref(), Some(&requested.command));
        assert_eq!(failure.requested_args, requested.args);
        assert_eq!(failure.error, error);
        assert!(
            failure
                .hint
                .as_deref()
                .is_some_and(|hint| hint.contains("~/.codex/config.toml"))
        );
        let markdown = session.transcript.to_markdown();
        assert!(markdown.contains("MCP startup failure"));
        assert!(markdown.contains("viewer-mcp"));
        assert!(markdown.contains("~/.codex/config.toml"));

        session.handle_event(AgentEvent::TurnFinished(Ok(StopReason::EndTurn)));
        let report = session.take_finished_turns().pop().unwrap();
        assert_eq!(report.tool_calls, 0);
        assert!(report.failed_tool_calls.is_empty());
    }

    #[test]
    fn codex_config_replacing_our_mcp_server_is_a_notice() {
        let mut session = AgentSession::default();
        let error = "[codex-acp] MCP server `rerun` was not started, because the Codex config \
                     already defines an MCP server with this name. Codex uses its own entry, \
                     or no server if that entry is disabled.";
        session.handle_event(AgentEvent::Update(SessionUpdate::ToolCall(
            ToolCall::new("mcp_startup.rerun.1", "mcp__rerun__startup")
                .status(ToolCallStatus::Failed)
                .content(vec![ToolCallContent::from(ContentBlock::Text(
                    TextContent::new(error),
                ))]),
        )));

        let TranscriptItem::Note { text, is_error } = &session.transcript.items[0].item else {
            panic!(
                "expected a note, got {:?}",
                session.transcript.items[0].item
            );
        };
        assert!(!is_error);
        assert_eq!(
            text,
            "Codex uses the `rerun` MCP server from `[mcp_servers.rerun]` in your Codex config \
             instead of the one Rerun provides."
        );
    }

    #[test]
    fn ordinary_failed_tools_are_not_classified_as_mcp_startup_failures() {
        let mut session = AgentSession::default();
        session.handle_event(AgentEvent::Update(SessionUpdate::ToolCall(
            ToolCall::new("call-1", "mcp__rerun__startup").status(ToolCallStatus::Failed),
        )));

        assert_matches!(
            session.transcript.items[0].item,
            TranscriptItem::ToolCall(_)
        );
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
        let mut session = AgentSession::default();
        session.turns.current = Some(TurnStart::testing(0));
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
