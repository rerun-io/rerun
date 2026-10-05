//! Agent failures that the host recognizes and reports with more context than a generic error.

use std::path::PathBuf;

use agent_client_protocol::schema::v1::{
    ContentBlock, SessionUpdate, ToolCallContent, ToolCallStatus,
};

use crate::McpStdioServer;

/// An MCP server failed while the agent was opening the session.
#[derive(Clone, Debug)]
pub struct McpStartupFailure {
    /// Server name from the synthetic startup event.
    pub server_name: String,

    /// Error reported by the agent or its adapter.
    pub error: String,

    /// Executable the host asked the agent to start, when this was a host-provided server.
    pub requested_command: Option<PathBuf>,

    /// Arguments belonging to [`Self::requested_command`].
    pub requested_args: Vec<String>,

    /// Actionable context inferred from a known adapter failure mode.
    pub hint: Option<String>,
}

impl McpStartupFailure {
    /// The host-requested command, quoted so it can be pasted into a shell.
    pub fn requested_command_line(&self) -> Option<String> {
        let command = self.requested_command.as_ref()?.to_string_lossy();
        Some(shell_words::join(std::iter::chain(
            std::iter::once(command.as_ref()),
            self.requested_args.iter().map(String::as_str),
        )))
    }

    /// Decodes codex-acp's synthetic failed tool call for an MCP server that never started:
    /// tool call id `mcp_startup.<name>`, title `mcp__<name>__startup`.
    ///
    /// ACP says nothing about MCP startup failures, and this convention is codex-acp's own,
    /// so every other agent yields `None` here and its updates reach the transcript unchanged.
    /// Recognizing it keeps the failure out of the turn report's tool-call counts and gives it a
    /// dedicated transcript item that can name the server definition we asked for.
    pub fn from_codex_update(
        update: &SessionUpdate,
        requested_servers: &[McpStdioServer],
    ) -> Option<Self> {
        let SessionUpdate::ToolCall(call) = update else {
            return None;
        };
        if call.status != ToolCallStatus::Failed || !call.tool_call_id.0.starts_with("mcp_startup.")
        {
            return None;
        }

        let server_name = call
            .title
            .strip_prefix("mcp__")?
            .strip_suffix("__startup")?
            .to_owned();
        let error = call
            .content
            .iter()
            .find_map(|content| match content {
                ToolCallContent::Content(content) => match &content.content {
                    ContentBlock::Text(text) => Some(text.text.clone()),
                    _ => None,
                },
                _ => None,
            })
            .or_else(|| {
                call.raw_output
                    .as_ref()
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| "The agent did not provide an error message.".to_owned());
        let requested = requested_servers
            .iter()
            .find(|server| server.name == server_name);
        let hint = error
            .contains("codex-acp forwarded startup error")
            .then(|| {
                format!(
                    "Codex ACP may have replaced this definition with `[mcp_servers.{server_name}]` \
                     from `~/.codex/config.toml`. Check that entry or update codex-acp so \
                     session-provided MCP servers take precedence."
                )
            });

        Some(Self {
            server_name,
            error,
            requested_command: requested.map(|server| server.command.clone()),
            requested_args: requested
                .map(|server| server.args.clone())
                .unwrap_or_default(),
            hint,
        })
    }
}
