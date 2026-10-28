//! Chat UI for driving a coding agent over the Agent Client Protocol (ACP).
//!
//! The protocol, session state, and transcript live in [`re_agent`]; this crate renders them
//! with egui and `re_ui`. Everything a host needs from `re_agent` is re-exported here.

mod ui;

pub use re_agent::{
    AgentCommand, AgentEntry, AgentEvent, AgentHarnessConfig, AgentProfile, AgentProfileId,
    AgentSession, AgentSettings, AuthPrompt, LineDirection, LogLine, McpServerConfig,
    McpStdioServer, PendingElicitation, PendingPermission, Phase, Prompt, PromptImage,
    SessionContext, ToolCallState, Transcript, TranscriptEntry, TranscriptItem, TurnOutcome,
    TurnReport, acp, find_executable, message_text, truncate_bytes,
};
pub use ui::{AgentPanel, RECOMMENDED_WIDTH};
