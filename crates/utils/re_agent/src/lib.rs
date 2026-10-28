//! Talk to a coding agent over the Agent Client Protocol (ACP).
//!
//! The agent runs behind a backend, e.g. one that runs an agent harness (an agent program such as
//! Claude Code or Codex) as a subprocess.
//! [`AgentSession`] turns its events into a [`Transcript`] and tracks what the agent is waiting
//! for. Nothing here depends on a UI toolkit: a host polls the session and renders it however it
//! likes, or drives it headless.
//!
//! # The agent stays the user's agent
//!
//! This crate is a layer on top of the agent the user already has, not a replacement for it.
//! The agent starts with its own default system prompt and loads its usual configuration from
//! the working directory and the user's home, exactly as it would in a terminal:
//! Claude Code reads `CLAUDE.md`, `.claude/settings*.json`, skills, slash commands, and the
//! MCP servers configured there; Codex reads `AGENTS.md` and its config, and so on.
//!
//! What the host adds comes on top of that:
//! [`SessionContext::preamble`] is sent as an extra text block with the first prompt, and
//! [`McpServerConfig`]s are handed over as additional MCP servers when the session opens.
//! Neither removes anything the agent would otherwise have.
//! Unless the user has picked a model, the session also switches to a fast one if the agent
//! offers it (see [`AgentHarnessConfig::model_preferences`]).

#[cfg(agent_harness_backend)]
mod agent_harness_backend;
mod agent_harness_config;
mod backend;
mod elicitation;
mod error;
mod profiles;
mod responder;
mod session;
mod settings;
mod transcript;
mod turn;

/// The Agent Client Protocol vocabulary that every backend speaks.
pub mod acp {
    pub use agent_client_protocol_schema as schema;
}

pub use agent_harness_config::{AgentHarnessConfig, McpStdioServer};
pub use backend::{AgentCommand, AgentEvent, LineDirection};
pub use error::McpStartupFailure;
pub use profiles::{AgentEntry, AgentProfile, AgentProfileId, find_executable};
pub use responder::{AgentStoppedWaiting, Responder};
pub use session::{
    AgentSession, AuthPrompt, LogLine, PendingElicitation, PendingPermission, Phase, Prompt,
    PromptImage,
};
pub use settings::{AgentSettings, McpServerConfig, SessionContext};
pub use transcript::{
    ToolCallState, Transcript, TranscriptEntry, TranscriptItem, message_text, model_name,
    truncate_bytes,
};
pub use turn::{TurnOutcome, TurnReport};
