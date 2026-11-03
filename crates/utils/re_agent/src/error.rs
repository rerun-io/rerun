//! Agent failures that the host recognizes and reports with more context than a generic error.

use std::path::PathBuf;

use agent_client_protocol_schema::v1::{
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

    /// A notice to show instead of this failure, if codex-acp did not start our server because
    /// the user's Codex config defines one with the same name, which Codex runs instead.
    ///
    /// For `rerun` that is usually `rerun viewer-mcp` from PATH, which still works
    /// but might be using a different version of `rerun`.
    pub fn replaced_by_agent_config_note(&self) -> Option<String> {
        let Self {
            server_name, error, ..
        } = self;
        error
            .contains("the Codex config already defines an MCP server with this name")
            .then(|| {
                format!(
                    "Codex uses the `{server_name}` MCP server from `[mcp_servers.{server_name}]` in your Codex config instead of the one Rerun provides."
                )
            })
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

/// Who reported an [`AgentError`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentErrorSource {
    /// An agent such as Claude Code or `OpenCode`, as it introduced itself, if it did.
    Agent {
        /// The name the agent gave, e.g. `OpenCode`.
        name: Option<String>,

        /// The version the agent gave, e.g. `1.4.2`.
        version: Option<String>,
    },

    /// A model provider's API, called by our own agent loop.
    ModelApi {
        /// The provider's name, e.g. `Anthropic`.
        provider: Option<String>,
    },
}

/// What the agent was asked to do when it reported an [`AgentError`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailedRequest {
    /// Answer a prompt: the turn ended with the error.
    Prompt,

    /// Anything else, e.g. connecting or changing a setting.
    Other,
}

/// A request that the agent failed, e.g. a prompt it could not answer.
///
/// Unlike an error from the host, the user can only fix this by changing the agent, its
/// configuration, or what it is connected to, so it is shown along with who reported it.
#[derive(Clone, Debug)]
pub struct AgentError {
    /// Who reported the error.
    pub source: AgentErrorSource,

    /// What the agent failed to do.
    pub request: FailedRequest,

    /// What went wrong, as [`re_error::StructuredError`] text.
    pub message: String,
}

impl AgentError {
    /// Who reported the error, e.g. "`OpenCode` 1.4.2", or "the Anthropic API".
    pub fn reporter(&self) -> String {
        match &self.source {
            AgentErrorSource::Agent {
                name: Some(name),
                version: Some(version),
            } => format!("{name} {version}"),
            AgentErrorSource::Agent {
                name: Some(name),
                version: None,
            } => name.clone(),
            AgentErrorSource::Agent { name: None, .. } => "the agent".to_owned(),
            AgentErrorSource::ModelApi {
                provider: Some(provider),
            } => format!("the {provider} API"),
            AgentErrorSource::ModelApi { provider: None } => "the model API".to_owned(),
        }
    }

    /// What the user can do about the error.
    pub fn hint(&self) -> String {
        match &self.source {
            AgentErrorSource::Agent { name, .. } => {
                let agent = name.as_deref().unwrap_or("the agent");
                if self.message.contains("input_schema") {
                    format!(
                        "The model provider rejected a tool that {agent} offered it. \
                         Tools come from {agent} itself and from its MCP servers, including Rerun's. \
                         Update {agent} and Rerun. If that does not help, turn off the MCP servers \
                         of {agent} one at a time to find the faulty one."
                    )
                } else {
                    format!(
                        "This comes from {agent} or the model provider it calls. \
                         If it keeps happening, update {agent}, or choose another agent in the agent setup."
                    )
                }
            }
            AgentErrorSource::ModelApi { .. } => {
                "If it keeps happening, check your API key and account with the provider, \
                 or choose another model in the agent setup."
                    .to_owned()
            }
        }
    }
}
