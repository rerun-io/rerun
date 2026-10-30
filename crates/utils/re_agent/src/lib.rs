//! Talk to a coding agent over the Agent Client Protocol (ACP), or run one against a model API.
//!
//! The agent runs behind a backend: either one that runs an agent harness (an agent program such
//! as Claude Code or Codex) as a subprocess, or one that runs the agent loop itself and calls a
//! model API directly with the user's key.
//! [`AgentSession`] turns its events into a [`Transcript`] and tracks what the agent is waiting
//! for. Nothing here depends on a UI toolkit: a host polls the session and renders it however it
//! likes, or drives it headless.
//!
//! # The agent stays the user's agent
//!
//! With an agent harness ([`AgentLaunch::AgentHarness`]), this crate is a layer on top of the
//! agent the user already has, not a replacement for it.
//! The agent starts with its own default system prompt and loads its usual configuration from
//! the working directory and the user's home, exactly as it would in a terminal:
//! Claude Code reads `CLAUDE.md`, `.claude/settings*.json`, skills, slash commands, and the
//! MCP servers configured there; Codex reads `AGENTS.md` and its config, and so on.
//!
//! What the host adds comes on top of that:
//! [`SessionContext::agent_harness_preamble`] is sent as an extra text block with the first prompt, and
//! [`McpServerConfig`]s are handed over as additional MCP servers when the session opens.
//! Neither removes anything the agent would otherwise have.
//! Unless the user has picked a model, the session also switches to a fast one if the agent
//! offers it (see [`AgentHarnessConfig::model_preferences`]).
//!
//! The model-API backend has none of that: no skills or configuration of the user's, only the
//! model, [`SessionContext::model_api_preamble`] as its system prompt, and the tools of the host's
//! [`AgentToolbox`].

// The workspace has two reqwest versions until it can move to 0.13; see the root `Cargo.toml`.
extern crate reqwest013 as reqwest;

mod agent_harness;
mod agent_toolbox;
mod backend;
mod elicitation;
mod error;
mod model_api_backend;
mod redactor;
mod responder;
mod session;
mod settings;
mod transcript;
mod turn;

/// Whether the user types model-API keys into the agent panel, which keeps them with its settings.
///
/// Only on the web, which has no environment variables. The desktop reads keys from the
/// environment only, since there is no secure storage for typed-in keys there yet.
pub const API_KEYS_IN_SETTINGS: bool = cfg!(target_arch = "wasm32");

/// The Agent Client Protocol vocabulary that every backend speaks.
pub mod acp {
    pub use agent_client_protocol_schema as schema;
}

#[cfg(agent_harness_backend)]
pub use agent_harness::find_executable;
pub use agent_harness::{
    AgentEntry, AgentHarnessConfig, AgentKind, AgentProfile, AgentProfileId, McpStdioServer,
};
pub use agent_toolbox::{AgentToolbox, ToolDefinition, ToolOutput};
pub use backend::{AgentCommand, AgentEvent, LineDirection};
pub use error::McpStartupFailure;
pub use model_api_backend::{ApiKey, HostedTools, ModelApiConfig, ModelApiError, ModelApiProvider};
pub use redactor::{OnRedacted, RedactionAgent, RedactionError, Redactor};
pub use responder::{AgentStoppedWaiting, Responder};
pub use session::{
    AgentSession, AuthPrompt, ElicitationId, LogLine, PendingElicitation, PendingPermission, Phase,
    Prompt, PromptImage,
};
pub use settings::{AgentLaunch, AgentLaunchError, AgentSettings, McpServerConfig, SessionContext};
pub use transcript::{
    ToolCallState, Transcript, TranscriptEntry, TranscriptItem, message_text, model_name,
    truncate_bytes,
};
pub use turn::{TurnOutcome, TurnReport};
