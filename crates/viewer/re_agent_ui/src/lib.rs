//! Chat UI for driving a coding agent over the Agent Client Protocol (ACP).
//!
//! The protocol, session state, and transcript live in [`re_agent`]; this crate renders them
//! with egui and `re_ui`. Everything a host needs from `re_agent` is re-exported here.

mod ui;

pub use re_agent::{
    AgentCommand, AgentConnection, AgentEntry, AgentEvent, AgentProfile, AgentSession,
    AgentSettings, AuthPrompt, LaunchConfig, LogLine, McpServerConfig, McpStdioServer,
    PendingPermission, Phase, SessionContext, ToolCallState, Transcript, TranscriptEntry,
    TranscriptItem, acp, find_executable,
};
pub use ui::AgentPanel;
