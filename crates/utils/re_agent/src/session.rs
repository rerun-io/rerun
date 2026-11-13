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
    SessionConfigId, SessionConfigOptionValue, SessionId, SessionUpdate, StopReason, TextContent,
    ToolCallId, ToolCallStatus, ToolKind,
};

use re_async::{AsyncRuntimeHandle, UiWaker};
use web_time::Instant;

use crate::AgentToolbox;
use crate::agent_harness::McpStdioServer;
use crate::backend::{AgentCommand, AgentConnection, AgentEvent, LineDirection, SessionOrigin};
use crate::elicitation::response_note;
use crate::error::{AgentError, AgentErrorSource, FailedRequest, McpStartupFailure};
use crate::reports::{McpStartupReport, SessionReports};
use crate::responder::{AgentStoppedWaiting, Responder};
use crate::settings::AgentLaunch;
use crate::startup::{AgentBackendKind, StartupOutcome, StartupStart};
use crate::transcript::{ToolCallState, Transcript, TranscriptItem, message_text, truncate_bytes};
use crate::turn::{TurnOutcome, TurnReport, TurnStart, describe_failed_tool_call};

const MAX_LOG_LINES: usize = 500;

/// How many of the last stderr lines may explain why the agent exited.
const EXIT_REASON_STDERR_LINES: usize = 10;

/// A stderr line can be huge (e.g. minified JSON), but the exit reason is shown in a narrow panel.
const MAX_EXIT_REASON_DETAIL_BYTES: usize = 300;

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
        let Self {
            request: _,
            responder,
        } = self;
        let outcome = match option_id {
            Some(option_id) => {
                RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(option_id))
            }
            None => RequestPermissionOutcome::Cancelled,
        };
        match responder {
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

    /// The host's tools. Calls that reach them through [`AgentToolbox::mcp_server_name`] and that
    /// need no approval there are allowed without asking.
    toolbox: Option<Arc<dyn AgentToolbox>>,
}

/// What the agent reported about itself, and what was asked of it.
#[derive(Default)]
struct AgentInfo {
    /// How the agent runs, once it has been started.
    backend: Option<AgentBackendKind>,

    /// The session the agent opened, once it has.
    session_id: Option<SessionId>,

    name: Option<String>,
    implementation_id: Option<String>,

    /// The version the agent reported, which is ours for a model-API agent.
    version: Option<String>,

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

/// What a loaded session replayed of its history.
#[derive(Default)]
struct History {
    /// The agent is replaying it, so its prompts must be added to the transcript.
    replaying: bool,

    /// How many transcript items it filled, at the start of the transcript.
    len: usize,
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

    /// The start of the agent, until the session opens or the agent goes away.
    startup: Option<StartupStart>,

    /// The history of an earlier session that is being loaded.
    history: History,

    /// Since the host last called [`Self::take_reports`].
    reports: SessionReports,

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
                session.startup = Some(StartupStart::new(AgentBackendKind::AgentHarness));
                session.agent.backend = Some(AgentBackendKind::AgentHarness);
                session.phase = Phase::Connecting {
                    status: format!("Starting {}…", config.command.display()),
                };
                session.launch.mcp_servers = config.mcp_servers.clone();
                session.launch.preamble = config.preamble.clone();
                session.launch.readable_directories = config.additional_directories.clone();
                session.launch.off_limits_directories = config.off_limits_directories.clone();
                session.launch.cwd = config.cwd.clone();
                cfg_select! {
                    agent_harness_backend => Ok(crate::agent_harness::spawn(*config, waker)),
                    _ => {{
                        drop((config, waker));
                        Err(StartError::HarnessUnsupported)
                    }}
                }
            }
            AgentLaunch::ModelApi(config) => {
                session.startup = Some(StartupStart::new(AgentBackendKind::ModelApi));
                session.agent.backend = Some(AgentBackendKind::ModelApi);
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
                session.finish_startup(StartupOutcome::Failed);
            }
        }
        session
    }

    /// Ends the session and returns every report that the host has not taken yet,
    /// including the running turn, reported as aborted.
    pub fn into_reports(mut self) -> SessionReports {
        self.finish_turn(TurnOutcome::Aborted, "Stopped");
        self.finish_startup(StartupOutcome::Stopped);
        std::mem::take(&mut self.reports)
    }

    /// Answer every permission request with the first "allow" option, without asking.
    /// Meant for unattended runs; the agent gets to do anything it asks for.
    pub fn set_auto_approve(&mut self, auto_approve: bool) {
        self.auto_approve = auto_approve;
    }

    /// The host's tools, for judging an agent harness' permission requests to call them over MCP.
    /// See [`AgentToolbox::mcp_server_name`].
    pub fn set_toolbox(&mut self, toolbox: Option<Arc<dyn AgentToolbox>>) {
        self.launch.toolbox = toolbox;
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

    /// Whether a replayed prompt block is the preamble we sent ahead of the first prompt.
    ///
    /// Compared by its first line only, since the rest can differ between viewer runs, e.g. the
    /// viewer's address.
    ///
    /// Whitespace is collapsed first, since some agents (e.g. Codex) do that to the prompt they
    /// derive a session title from.
    fn is_preamble(&self, text: &str) -> bool {
        let collapse = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
        self.launch
            .preamble
            .as_deref()
            .and_then(|preamble| preamble.lines().next())
            .map(collapse)
            .is_some_and(|first_line| {
                !first_line.is_empty() && collapse(text).starts_with(&first_line)
            })
    }

    fn push_agent_error(&mut self, request: FailedRequest, message: String) {
        let AgentInfo {
            backend,
            name,
            version,
            ..
        } = &self.agent;
        let source = match backend {
            Some(AgentBackendKind::ModelApi) => AgentErrorSource::ModelApi {
                provider: name.clone(),
            },
            Some(AgentBackendKind::AgentHarness) | None => AgentErrorSource::Agent {
                name: name.clone(),
                version: version.clone(),
            },
        };
        self.transcript.push_agent_error(AgentError {
            source,
            request,
            message,
        });
    }

    /// The prompt to send again after it failed, and the transcript index of its error.
    ///
    /// Only the error that ended the last turn qualifies: an earlier prompt has been superseded,
    /// and only an idle session can take the prompt again.
    pub fn retryable_prompt(&self) -> Option<(usize, &Prompt)> {
        if !self.is_ready() {
            return None;
        }
        let items = &self.transcript.items;
        let error_index = items.iter().rposition(|entry| {
            matches!(
                &entry.item,
                TranscriptItem::AgentError(error) if error.request == FailedRequest::Prompt
            )
        })?;
        let user_index = items
            .iter()
            .rposition(|entry| matches!(entry.item, TranscriptItem::User(_)))?;
        if error_index < user_index {
            return None;
        }
        Some((error_index, items.get(user_index)?.item.user_prompt()?))
    }

    /// What the agent calls itself, once it has told us.
    pub fn agent_name(&self) -> Option<&str> {
        self.agent.name.as_deref()
    }

    /// The implementation identifier from the ACP handshake, independent of its display title.
    pub fn agent_implementation_id(&self) -> Option<&str> {
        self.agent.implementation_id.as_deref()
    }

    /// The running agent's self-reported version, rather than a separately installed CLI's version.
    pub fn agent_version(&self) -> Option<&str> {
        self.agent.version.as_deref()
    }

    pub fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    /// The id the agent gave the session, once it is open.
    ///
    /// Pass it as [`crate::AgentHarnessConfig::session_to_load`] to continue the session later.
    pub fn session_id(&self) -> Option<&SessionId> {
        self.agent.session_id.as_ref()
    }

    /// The working directory of an agent harness. Agents find a session to load by it.
    pub fn cwd(&self) -> &Path {
        &self.launch.cwd
    }

    /// How many items at the start of the transcript were replayed from a loaded session.
    ///
    /// They carry the time of the replay, not of the original messages.
    pub fn history_len(&self) -> usize {
        self.history.len
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

    /// Why the agent exited.
    ///
    /// The agent usually explains it on stderr, so that is quoted as a detail.
    fn exit_reason(&self) -> re_error::StructuredError {
        let last_lines: Vec<String> = self
            .log
            .iter()
            .rev()
            .filter(|log_line| log_line.direction == LineDirection::Stderr)
            .map(|log_line| strip_ansi_escapes(&log_line.line).trim().to_owned())
            .filter(|line| !line.is_empty())
            .take(EXIT_REASON_STDERR_LINES)
            .collect();
        // Agents often print warnings after the error, e.g. Node's deprecation warnings.
        // Errors from long before the exit are not why it exited, hence only the last few lines.
        let explanation = last_lines
            .iter()
            .find(|line| reports_failure(line))
            .or_else(|| last_lines.first());

        let mut reason = re_error::StructuredError::from_summary("The agent exited");
        if let Some(explanation) = explanation {
            reason.add_detail(truncate_bytes(explanation, MAX_EXIT_REASON_DETAIL_BYTES));
        }
        reason
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

    /// Drains what the session reported since the last call.
    pub fn take_reports(&mut self) -> SessionReports {
        std::mem::take(&mut self.reports)
    }

    /// Reports how the start of the agent ended, unless it already did.
    fn finish_startup(&mut self, outcome: StartupOutcome) {
        let Some(startup) = self.startup.take() else {
            return;
        };
        let model = (outcome == StartupOutcome::Ready)
            .then(|| self.transcript.current_model().map(str::to_owned))
            .flatten();
        self.reports
            .startups
            .push(startup.finish(self.agent.name.clone(), model, outcome));
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
                TranscriptItem::AgentError(error) => errors.push(error.message.clone()),
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

        self.reports.turns.push(TurnReport {
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

    /// Asks the agent to change a session setting; its confirmed value arrives in a config update.
    pub fn set_config_option(
        &self,
        config_id: SessionConfigId,
        value: impl Into<SessionConfigOptionValue>,
    ) {
        if let Some(connection) = &self.connection {
            connection.send(AgentCommand::SetConfigOption {
                config_id,
                value: value.into(),
            });
        }
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
                self.agent.version = agent_info.as_ref().map(|info| info.version.clone());
                self.agent.implementation_id = agent_info.as_ref().map(|info| info.name.clone());
                self.agent.name = agent_info.map(|info| info.title.unwrap_or(info.name));
                self.agent.prompt_capabilities = capabilities.prompt_capabilities;
            }
            AgentEvent::LoadingSession => {
                self.phase = Phase::Connecting {
                    status: "Loading session…".to_owned(),
                };
                self.transcript = Transcript::default();
                self.history = History {
                    replaying: true,
                    len: 0,
                };
            }
            AgentEvent::SessionStarted {
                session_id,
                config_options,
                origin,
            } => {
                let was_replaying = std::mem::take(&mut self.history.replaying);
                if origin == SessionOrigin::Loaded {
                    // A turn the earlier run quit in the middle of never finishes.
                    self.transcript
                        .fail_unfinished_tool_calls("Interrupted when the conversation was closed");
                    self.history.len = self.transcript.items.len();
                } else if was_replaying {
                    // The agent knows nothing of what it replayed before failing.
                    self.transcript = Transcript::default();
                    self.transcript.push_note(
                        "The agent could not continue the earlier conversation, so this is a new one",
                        false,
                    );
                }
                self.agent.session_id = Some(session_id);
                self.transcript.config_options = config_options;
                self.agent.auth = None;
                self.phase = Phase::Ready;
                self.finish_startup(StartupOutcome::Ready);
            }
            AgentEvent::AuthRequired { methods, message } => {
                self.history.replaying = false;
                self.phase = Phase::Connecting {
                    status: "Waiting for login".to_owned(),
                };
                self.agent.auth = Some(AuthPrompt { methods, message });
                if let Some(startup) = &mut self.startup {
                    startup.auth_required = true;
                }
            }
            AgentEvent::Update(update) => {
                if let Some(failure) =
                    McpStartupFailure::from_update(&update, &self.launch.mcp_servers)
                {
                    self.reports.mcp_startup_failures.push(McpStartupReport {
                        agent: self.agent.name.clone(),
                        failure: failure.clone(),
                    });
                    if let Some(note) = failure.replaced_by_agent_config_note() {
                        self.transcript.push_note(note, false);
                    } else {
                        self.transcript.push_mcp_startup_failure(failure);
                    }
                } else if let SessionUpdate::SessionInfoUpdate(info) = &update
                    && let agent_client_protocol_schema::MaybeUndefined::Value(title) = &info.title
                    && self.is_preamble(title)
                {
                    // Agents that name a session after its first prompt would name it after our
                    // preamble, so we derive the title ourselves instead.
                } else if let SessionUpdate::UserMessageChunk(chunk) = &update {
                    // Live prompts are added as they are sent, so only a replayed history needs
                    // the agent's copy.
                    if self.history.replaying
                        && let ContentBlock::Text(TextContent { text, .. }) = &chunk.content
                        && !self.is_preamble(text)
                    {
                        self.transcript.push_replayed_user_text(text);
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
                let is_harmless = self
                    .transcript
                    .tool_call(&pending.request.tool_call.tool_call_id)
                    .is_some_and(|call| {
                        is_read_within(call, &self.launch.readable_directories)
                            || self.launch.toolbox.as_deref().is_some_and(|toolbox| {
                                is_toolbox_call_without_approval(
                                    call,
                                    toolbox,
                                    &self.launch.mcp_servers,
                                )
                            })
                    });
                if (self.auto_approve || is_harmless)
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
                        self.push_agent_error(FailedRequest::Prompt, err);
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
            AgentEvent::Error(err) => self.push_agent_error(FailedRequest::Other, err),
            AgentEvent::Disconnected => {
                self.finish_turn(TurnOutcome::Aborted, "The agent exited before finishing");
                self.finish_startup(StartupOutcome::Exited);
                self.phase = Phase::Disconnected {
                    reason: self.exit_reason().to_string(),
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

/// Whether `call` goes to a tool of `toolbox` through the MCP server the host supplied for it,
/// and `toolbox` would run that call without approval.
///
/// The tool is named by the call's title, `mcp__<server>__<tool>`, as Claude Code reports it.
fn is_toolbox_call_without_approval(
    call: &ToolCallState,
    toolbox: &dyn AgentToolbox,
    mcp_servers: &[McpStdioServer],
) -> bool {
    let Some(server) = toolbox.mcp_server_name() else {
        return false;
    };
    if !mcp_servers.iter().any(|supplied| supplied.name == server) {
        return false;
    }
    let Some(tool) = call
        .title
        .strip_prefix("mcp__")
        .and_then(|rest| rest.strip_prefix(server))
        .and_then(|rest| rest.strip_prefix("__"))
    else {
        return false;
    };
    let no_arguments = serde_json::Map::new();
    let arguments = call
        .raw_input
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .unwrap_or(&no_arguments);
    !toolbox.requires_approval(tool, arguments)
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

/// Whether a stderr line reports a failure, e.g. `error: …`, `TypeError: …`, or a Rust panic.
///
/// Matches whole words, so that `0 errors` or `errorHandler.js` do not count.
fn reports_failure(line: &str) -> bool {
    line.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .any(|word| {
            word.ends_with("error")
                || word.ends_with("exception")
                || matches!(word.as_str(), "fatal" | "panic" | "panicked")
        })
}

/// Removes terminal escape sequences, e.g. the colors of a CLI's error message.
fn strip_ansi_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.next() {
            // CSI, e.g. colors: ends at its first char in `@`..=`~`.
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            // OSC, e.g. hyperlinks and window titles: ends at BEL or `ESC \`.
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                        break;
                    }
                }
            }
            // Character set selection, e.g. `ESC ( B` from `tput sgr0`.
            Some('(' | ')' | '*' | '+') => {
                chars.next();
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use agent_client_protocol_schema::v1::{
        ContentChunk, SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectOption,
        ToolCall, ToolCallContent, ToolCallLocation, ToolCallUpdate, ToolCallUpdateFields,
    };

    use super::*;

    fn user_chunk(text: &str) -> AgentEvent {
        AgentEvent::Update(SessionUpdate::UserMessageChunk(ContentChunk::new(
            ContentBlock::Text(TextContent::new(text.to_owned())),
        )))
    }

    fn session_started(origin: SessionOrigin) -> AgentEvent {
        AgentEvent::SessionStarted {
            session_id: SessionId::new("earlier"),
            config_options: Vec::new(),
            origin,
        }
    }

    fn user_prompts(session: &AgentSession) -> Vec<&str> {
        session
            .transcript
            .items
            .iter()
            .filter_map(|entry| entry.item.user_prompt())
            .map(|prompt| prompt.text.as_str())
            .collect()
    }

    #[test]
    fn a_loaded_session_replays_prompts_without_the_preamble() {
        let mut session = AgentSession::default();
        session.launch.preamble = Some("You run in the viewer.\nIt is at :9876.".to_owned());
        session
            .transcript
            .push_note("Left over from before the load", false);
        session.handle_event(AgentEvent::LoadingSession);

        // The preamble of an earlier run, which named another port.
        session.handle_event(user_chunk("You run in the viewer.\nIt is at :1234."));
        session.handle_event(user_chunk("First question"));
        session.handle_event(user_chunk("with a second block"));
        session.handle_event(AgentEvent::Update(SessionUpdate::AgentMessageChunk(
            ContentChunk::new(ContentBlock::Text(TextContent::new("An answer"))),
        )));
        session.handle_event(user_chunk("Second question"));
        session.handle_event(session_started(SessionOrigin::Loaded));

        assert_eq!(
            user_prompts(&session),
            ["First question\n\nwith a second block", "Second question"]
        );
        assert_eq!(session.session_id(), Some(&SessionId::new("earlier")));
        assert_eq!(session.history_len(), 3);

        // Once the session is open, prompts are added as they are sent, not as echoed.
        session.handle_event(user_chunk("An echo"));
        assert_eq!(user_prompts(&session).len(), 2);
    }

    #[test]
    fn a_session_that_could_not_be_loaded_drops_what_it_replayed() {
        let mut session = AgentSession::default();
        session.handle_event(AgentEvent::LoadingSession);
        session.handle_event(user_chunk("A question the new session never saw"));
        session.handle_event(session_started(SessionOrigin::New));

        assert!(user_prompts(&session).is_empty());
        assert_eq!(session.history_len(), 0);
        assert!(session.transcript.items.iter().any(|entry| matches!(
            &entry.item,
            TranscriptItem::Note { text, is_error: false } if text.contains("earlier conversation")
        )));
    }

    /// A session title that an agent derived from our preamble is ignored.
    #[test]
    fn a_title_made_from_the_preamble_is_ignored() {
        let mut session = AgentSession::default();
        session.launch.preamble = Some("You run in\tthe viewer.\nIt is at :9876.".to_owned());
        let title = |title: &str| {
            AgentEvent::Update(SessionUpdate::SessionInfoUpdate(
                agent_client_protocol_schema::v1::SessionInfoUpdate::new().title(title.to_owned()),
            ))
        };

        session.handle_event(title(
            "You run in the viewer. It is at :1234. What is on screen?",
        ));
        assert_eq!(session.transcript.title, None);

        session.handle_event(title("What is on screen?"));
        assert_eq!(
            session.transcript.title.as_deref(),
            Some("What is on screen?")
        );
    }

    /// An agent that cannot load sessions replays nothing, but the user is still told.
    #[test]
    fn a_session_that_was_not_loaded_says_so() {
        let mut session = AgentSession::default();
        session.handle_event(AgentEvent::LoadingSession);
        session.handle_event(session_started(SessionOrigin::New));

        assert!(session.transcript.items.iter().any(|entry| matches!(
            &entry.item,
            TranscriptItem::Note { text, is_error: false } if text.contains("earlier conversation")
        )));
    }

    #[test]
    fn strips_terminal_escapes() {
        for (input, expected) in [
            ("plain", "plain"),
            ("\u{1b}[1;31merror\u{1b}(B\u{1b}[m: x", "error: x"),
            (
                "\u{1b}]8;;https://x.y\u{1b}\\link\u{1b}]8;;\u{1b}\\ text",
                "link text",
            ),
            ("\u{1b}]0;title\u{7}after", "after"),
            ("cut off \u{1b}[31", "cut off "),
        ] {
            assert_eq!(strip_ansi_escapes(input), expected, "input: {input:?}");
        }
    }

    #[test]
    fn the_disconnect_reason_quotes_the_stderr_line_that_explains_it() {
        use LineDirection::{Stderr, Stdout};

        let disconnect_reason = |lines: &[(LineDirection, &str)]| {
            let mut session = AgentSession::default();
            for &(direction, line) in lines {
                session.handle_event(AgentEvent::Log {
                    direction,
                    line: line.to_owned(),
                });
            }
            session.handle_event(AgentEvent::Disconnected);
            match session.phase() {
                Phase::Disconnected { reason } => reason.clone(),
                phase => panic!("Expected a disconnect, got {phase:?}"),
            }
        };

        assert_eq!(disconnect_reason(&[]), "The agent exited");
        assert_eq!(disconnect_reason(&[(Stdout, "{}")]), "The agent exited");
        assert_eq!(
            disconnect_reason(&[(Stderr, "starting"), (Stderr, "stopped"), (Stderr, "  ")]),
            "The agent exited\n- stopped",
            "Without an error, the last non-empty line explains the exit"
        );
        assert_eq!(
            disconnect_reason(&[
                (Stderr, "\u{1b}[31merror\u{1b}[0m: unknown option '--acp'"),
                (Stderr, "(node:42) DeprecationWarning: punycode"),
                (Stdout, "{}"),
            ]),
            "The agent exited\n- error: unknown option '--acp'",
            "A warning after the error must not hide it"
        );

        let mut stale_error = vec![(Stderr, "error: retrying the MCP server")];
        stale_error.extend([(Stderr, "0 errors"); EXIT_REASON_STDERR_LINES]);
        assert_eq!(
            disconnect_reason(&stale_error),
            "The agent exited\n- 0 errors",
            "An error long before the exit does not explain it"
        );

        let long_line = format!("Error: {}", "ü".repeat(MAX_EXIT_REASON_DETAIL_BYTES));
        assert_eq!(
            disconnect_reason(&[(Stderr, &long_line)]),
            format!(
                "The agent exited\n- {}",
                truncate_bytes(&long_line, MAX_EXIT_REASON_DETAIL_BYTES)
            ),
            "Long lines are cut"
        );
    }

    #[test]
    fn a_login_during_a_load_replays_the_history_only_once() {
        let mut session = AgentSession::default();
        session.handle_event(AgentEvent::LoadingSession);
        session.handle_event(user_chunk("First question"));
        session.handle_event(AgentEvent::AuthRequired {
            methods: Vec::new(),
            message: "Log in".to_owned(),
        });
        session.handle_event(user_chunk("Not part of any replay"));

        session.handle_event(AgentEvent::LoadingSession);
        session.handle_event(user_chunk("First question"));
        session.handle_event(session_started(SessionOrigin::Loaded));
        assert_eq!(user_prompts(&session), ["First question"]);
    }

    #[test]
    fn a_turn_the_earlier_run_quit_during_does_not_spin_forever() {
        let mut session = AgentSession::default();
        session.handle_event(AgentEvent::LoadingSession);
        session.handle_event(AgentEvent::Update(SessionUpdate::ToolCall(
            ToolCall::new("call-1", "Read a file").status(ToolCallStatus::InProgress),
        )));
        session.handle_event(session_started(SessionOrigin::Loaded));
        let status = session
            .transcript
            .tool_call(&ToolCallId::new("call-1"))
            .map(|call| call.status);
        assert_eq!(status, Some(ToolCallStatus::Failed));
    }

    #[test]
    fn startup_report() {
        let starting = || {
            let mut session = AgentSession::default();
            session.agent.name = Some("Test Agent".to_owned());
            session.startup = Some(StartupStart::new(AgentBackendKind::AgentHarness));
            session
        };
        let session_started = || AgentEvent::SessionStarted {
            session_id: "session".into(),
            config_options: vec![
                SessionConfigOption::select(
                    "model",
                    "Model",
                    "opus",
                    vec![SessionConfigSelectOption::new("opus", "Opus 5.5")],
                )
                .category(SessionConfigOptionCategory::Model),
            ],
            origin: SessionOrigin::New,
        };

        let mut session = starting();
        session.handle_event(AgentEvent::AuthRequired {
            methods: Vec::new(),
            message: String::new(),
        });
        session.handle_event(session_started());
        // Only the first outcome counts: the session is no longer starting.
        session.handle_event(AgentEvent::Disconnected);
        let reports = session.into_reports().startups;
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].agent.as_deref(), Some("Test Agent"));
        assert_eq!(reports[0].model.as_deref(), Some("Opus 5.5"));
        assert_eq!(reports[0].outcome, StartupOutcome::Ready);
        assert!(reports[0].auth_required);

        let mut session = starting();
        session.handle_event(AgentEvent::Disconnected);
        let reports = session.into_reports().startups;
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].model, None);
        assert_eq!(reports[0].outcome, StartupOutcome::Exited);
        assert!(!reports[0].auth_required);

        let reports = starting().into_reports().startups;
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].outcome, StartupOutcome::Stopped);
    }

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

    /// Only a failed prompt that is still the latest one can be retried, and only once the session
    /// is idle again; the retry carries the images of the original prompt.
    #[test]
    fn retryable_prompt() {
        let mut session = AgentSession::default();
        session.handle_event(AgentEvent::SessionStarted {
            session_id: agent_client_protocol_schema::v1::SessionId::new("test-session"),
            config_options: Vec::new(),
            origin: SessionOrigin::New,
        });
        let image = PromptImage {
            bytes: Arc::from([0_u8].as_slice()),
            mime_type: "image/png".to_owned(),
            resolution: [8, 6],
        };
        session.transcript.push_user(Prompt {
            text: "first".to_owned(),
            images: vec![image.clone()],
        });
        assert!(session.retryable_prompt().is_none(), "nothing failed yet");

        session.handle_event(AgentEvent::Error("Failed to set mode".to_owned()));
        assert!(
            session.retryable_prompt().is_none(),
            "a failed setting change is not a failed prompt"
        );

        session.handle_event(AgentEvent::TurnFinished(Err("boom".to_owned())));
        let (error_index, prompt) = session
            .retryable_prompt()
            .expect("the failed prompt can be retried");
        assert_eq!(error_index, 2);
        assert_eq!(prompt.text, "first");
        assert_eq!(prompt.images.len(), 1);
        assert!(Arc::ptr_eq(&prompt.images[0].bytes, &image.bytes));

        session.transcript.push_user("second".into());
        assert!(
            session.retryable_prompt().is_none(),
            "a newer prompt supersedes the failed one"
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
        let turns = session.into_reports().turns;
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
        assert!(session.take_reports().turns.is_empty());

        session.handle_event(AgentEvent::TurnFinished(Ok(StopReason::EndTurn)));
        assert!(!session.turn_in_progress());
        let reports = session.take_reports().turns;
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
        assert!(session.take_reports().turns.is_empty());

        // A turn cut short by the agent going away is still reported.
        session.begin_turn("again".into());
        session.handle_event(AgentEvent::Disconnected);
        let reports = session.take_reports().turns;
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

        let failures = session.take_reports().mcp_startup_failures;
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].failure.server_name, "rerun");
        assert!(session.take_reports().mcp_startup_failures.is_empty());

        session.handle_event(AgentEvent::TurnFinished(Ok(StopReason::EndTurn)));
        let report = session.take_reports().turns.pop().unwrap();
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

        let failures = session.take_reports().mcp_startup_failures;
        assert_eq!(failures.len(), 1);
        assert!(
            failures[0]
                .failure
                .replaced_by_agent_config_note()
                .is_some()
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

    /// What claude-agent-acp sends for a host-provided MCP server, verbatim.
    fn claude_code_mcp_status(status: &str, text: &str) -> SessionUpdate {
        let meta = serde_json::json!({
            "claudeCode": { "kind": "mcpServerStatus", "server": "rerun", "status": status },
        });
        SessionUpdate::AgentMessageChunk(
            ContentChunk::new(ContentBlock::Text(TextContent::new(text)))
                .meta(meta.as_object().cloned()),
        )
    }

    #[test]
    fn claude_code_mcp_startup_failures_are_reported() {
        let requested = McpStdioServer {
            name: "rerun".to_owned(),
            command: PathBuf::from("/Applications/Rerun.app/Contents/MacOS/rerun"),
            args: vec!["viewer-mcp".to_owned()],
        };
        let mut session = AgentSession::default();
        session.launch.mcp_servers = vec![requested.clone()];
        session.begin_turn("show the recording".into());
        session.handle_event(AgentEvent::Update(claude_code_mcp_status(
            "failed",
            "**MCP server \"rerun\" failed to start:** Its tools are unavailable in this \
             session. Run /mcp for the error.",
        )));

        let TranscriptItem::McpStartupFailure(failure) = &session.transcript.items[1].item else {
            panic!("expected a dedicated MCP startup failure");
        };
        assert_eq!(failure.requested_command.as_ref(), Some(&requested.command));
        assert_eq!(
            failure.error,
            "Its tools are unavailable in this session. Run /mcp for the error."
        );

        let failures = session.take_reports().mcp_startup_failures;
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].failure.server_name, "rerun");

        session.handle_event(AgentEvent::TurnFinished(Ok(StopReason::EndTurn)));
        let report = session.take_reports().turns.pop().unwrap();
        assert_eq!(report.response, "");
    }

    #[test]
    fn claude_code_mcp_needs_auth_is_not_a_startup_failure() {
        let mut session = AgentSession::default();
        session.handle_event(AgentEvent::Update(claude_code_mcp_status(
            "needs-auth",
            "**MCP server \"rerun\" needs authentication:** Its tools are unavailable until \
             the server is authenticated.",
        )));

        assert!(session.take_reports().mcp_startup_failures.is_empty());
        assert!(!matches!(
            session.transcript.items[0].item,
            TranscriptItem::McpStartupFailure(_)
        ));
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

    /// Approves only `get` without arguments, served by the `viewer` MCP server.
    struct TestToolbox;

    impl AgentToolbox for TestToolbox {
        fn definitions(&self) -> Vec<crate::ToolDefinition> {
            Vec::new()
        }

        fn requires_approval(
            &self,
            name: &str,
            arguments: &serde_json::Map<String, serde_json::Value>,
        ) -> bool {
            name != "get" || !arguments.is_empty()
        }

        fn mcp_server_name(&self) -> Option<&str> {
            Some("viewer")
        }

        fn call(
            &self,
            _name: &str,
            _arguments: serde_json::Map<String, serde_json::Value>,
        ) -> futures::future::BoxFuture<
            'static,
            Result<Vec<crate::ToolOutput>, Box<dyn std::error::Error + Send + Sync>>,
        > {
            Box::pin(std::future::ready(Ok(Vec::new())))
        }
    }

    #[test]
    fn toolbox_calls_over_mcp_follow_the_toolbox() {
        let servers = |names: &[&str]| {
            names
                .iter()
                .map(|name| McpStdioServer {
                    name: (*name).to_owned(),
                    command: PathBuf::from("viewer-mcp"),
                    args: Vec::new(),
                })
                .collect::<Vec<_>>()
        };
        let allowed = |title: &str, raw_input, servers: &[McpStdioServer]| {
            let call = ToolCallState {
                title: title.to_owned(),
                ..call(ToolKind::Other, &[], raw_input)
            };
            is_toolbox_call_without_approval(&call, &TestToolbox, servers)
        };
        let viewer = servers(&["viewer"]);

        assert!(allowed("mcp__viewer__get", None, &viewer));
        assert!(allowed(
            "mcp__viewer__get",
            Some(serde_json::json!({})),
            &viewer
        ));
        assert!(!allowed(
            "mcp__viewer__get",
            Some(serde_json::json!({ "x": 1 })),
            &viewer
        ));
        assert!(!allowed("mcp__viewer__set", None, &viewer));
        assert!(!allowed("mcp__other__get", None, &viewer));
        assert!(!allowed("mcp__viewerx__get", None, &viewer));
        assert!(!allowed("get", None, &viewer));
        assert!(
            !allowed("mcp__viewer__get", None, &servers(&["other"])),
            "the host did not supply the server, so the user configured it"
        );
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
