//! Chat UI for driving a coding agent over the Agent Client Protocol (ACP).
//!
//! The protocol, session state, and transcript live in [`re_agent`]; this crate renders them
//! with egui and `re_ui`. Everything a host needs from `re_agent` is re-exported here.
//!
//! This crate is a generic chat UI that any egui app could use, and must not know about the
//! Rerun Viewer: what ties the agent to the viewer goes in `re_agent_panel`.
//! `scripts/check_crate_layers.py` enforces this.

mod ui;

#[cfg(agent_harness_backend)]
pub use re_agent::find_executable;
pub use re_agent::{
    AgentCommand, AgentEntry, AgentEvent, AgentHarnessConfig, AgentLaunch, AgentLaunchError,
    AgentProfile, AgentProfileId, AgentSession, AgentSettings, AgentToolbox, ApiKey, AuthPrompt,
    HostedTools, LineDirection, LogLine, McpServerConfig, McpStdioServer, ModelApiConfig,
    ModelApiProvider, PendingElicitation, PendingPermission, Phase, Prompt, PromptImage,
    SessionContext, ToolCallState, ToolDefinition, ToolOutput, Transcript, TranscriptEntry,
    TranscriptItem, TurnOutcome, TurnReport, acp, message_text, truncate_bytes,
};
pub use ui::{AgentPanel, RECOMMENDED_WIDTH};
