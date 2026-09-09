//! Talk to a coding agent over the Agent Client Protocol (ACP).
//!
//! The agent runs as a subprocess spawned by [`AgentConnection`].
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

mod connection;
mod profiles;
mod session;
mod settings;
mod transcript;

pub use agent_client_protocol as acp;

pub use connection::{AgentCommand, AgentConnection, AgentEvent, LaunchConfig, McpStdioServer};
pub use profiles::{AgentEntry, AgentProfile, find_executable};
pub use session::{AgentSession, AuthPrompt, LogLine, PendingPermission, Phase};
pub use settings::{AgentSettings, McpServerConfig, SessionContext};
pub use transcript::{ToolCallState, Transcript, TranscriptEntry, TranscriptItem};
