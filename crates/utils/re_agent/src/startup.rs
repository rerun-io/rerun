//! Reports describing how agents started.

use std::time::Duration;

use web_time::Instant;

/// Which kind of agent a session runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentBackendKind {
    /// An agent (Claude Code, Codex, …) spawned as a subprocess and spoken to over ACP.
    AgentHarness,

    /// An agent loop we run ourselves, calling a model provider's API directly.
    ModelApi,
}

impl AgentBackendKind {
    pub fn name(&self) -> &'static str {
        match self {
            Self::AgentHarness => "agent_harness",
            Self::ModelApi => "model_api",
        }
    }
}

/// Bookkeeping for a start in progress, turned into a [`StartupReport`] when it ends.
pub struct StartupStart {
    pub backend: AgentBackendKind,
    pub started: Instant,
    pub auth_required: bool,
}

impl StartupStart {
    pub fn new(backend: AgentBackendKind) -> Self {
        Self {
            backend,
            started: Instant::now(),
            auth_required: false,
        }
    }

    pub fn finish(
        self,
        agent: Option<String>,
        model: Option<String>,
        outcome: StartupOutcome,
    ) -> StartupReport {
        let Self {
            backend,
            started,
            auth_required,
        } = self;
        StartupReport {
            backend,
            agent,
            model,
            duration: started.elapsed(),
            outcome,
            auth_required,
        }
    }
}

/// How an attempt to open a session ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupOutcome {
    /// The session opened, and prompts can be sent.
    Ready,

    /// The agent could not be started at all.
    Failed,

    /// The agent exited before the session opened.
    Exited,

    /// The host stopped the agent before the session opened, e.g. because the user restarted it.
    Stopped,
}

impl StartupOutcome {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Exited => "exited",
            Self::Stopped => "stopped",
        }
    }
}

/// How one start of an agent went, for hosts that want to log or record it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartupReport {
    /// Which kind of agent was started.
    pub backend: AgentBackendKind,

    /// The agent's self-reported name, if it had introduced itself by then.
    pub agent: Option<String>,

    /// The model the agent reported when the session opened, e.g. "Opus 5.5".
    ///
    /// `None` unless [`Self::outcome`] is [`StartupOutcome::Ready`], or when the agent does not
    /// report its model.
    pub model: Option<String>,

    /// From starting the agent to [`Self::outcome`].
    pub duration: Duration,

    pub outcome: StartupOutcome,

    /// Whether the agent asked the user to log in before [`Self::outcome`].
    pub auth_required: bool,
}
