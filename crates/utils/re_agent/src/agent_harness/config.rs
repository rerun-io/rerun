use std::path::PathBuf;

use agent_client_protocol_schema::v1::{SessionConfigValueId, SessionModeId};

/// An MCP server the agent spawns itself over stdio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpStdioServer {
    /// The name the agent sees the server under.
    pub name: String,

    /// Executable the agent runs to start the server.
    pub command: PathBuf,

    /// Arguments passed to [`Self::command`].
    pub args: Vec<String>,
}

/// Everything needed to spawn an agent and open a session with it.
#[derive(Clone, Debug)]
pub struct AgentHarnessConfig {
    /// The agent executable, or an adapter such as `npx`.
    pub command: PathBuf,

    /// Arguments passed to [`Self::command`].
    pub args: Vec<String>,

    /// Environment variables set for the agent process, on top of the inherited ones.
    pub env: Vec<(String, String)>,

    /// Working directory of the agent process and of the session.
    pub cwd: PathBuf,

    /// Extra directories the agent may read and edit, e.g. one holding skills.
    /// Read-only tool calls that stay inside them are allowed without asking the user.
    pub additional_directories: Vec<PathBuf>,

    /// Directories the preamble asks the agent to stay out of, only checked after the fact.
    /// See [`crate::SessionContext::off_limits_directories`].
    pub off_limits_directories: Vec<PathBuf>,

    /// MCP servers the agent spawns and connects to when the session opens.
    pub mcp_servers: Vec<McpStdioServer>,

    /// Forward every JSON-RPC line as [`crate::AgentEvent::Log`]. Stderr is always forwarded.
    pub log_protocol: bool,

    /// Markdown sent together with the first prompt of the session.
    pub preamble: Option<String>,

    /// Models to prefer for this session, best first.
    ///
    /// Each entry is matched, case-insensitively, as a substring of the id and the name of every
    /// model the agent offers over ACP; the first hit is selected before the session takes its
    /// first prompt. An agent that offers no model selector, or nothing that matches, keeps
    /// whatever model it would use on its own.
    pub model_preferences: Vec<String>,

    /// Exact saved model ID, restored only if the agent still offers it.
    pub preferred_model: Option<SessionConfigValueId>,

    /// Exact session mode id to restore when the agent offers it.
    pub preferred_mode: Option<SessionModeId>,
}
