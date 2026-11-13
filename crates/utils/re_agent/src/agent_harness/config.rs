use std::path::PathBuf;

use agent_client_protocol_schema::v1::SessionId;

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

    /// Ranked model-name fallbacks used when no saved model selection is available
    /// or the agent no longer offers it.
    ///
    /// Each entry is matched, case-insensitively, as a substring of the id and the name of every
    /// model the agent offers over ACP; the first hit is selected before the session takes its
    /// first prompt. An agent that offers no model selector, or nothing that matches, keeps
    /// whatever model it would use on its own.
    pub model_preferences: Vec<String>,

    /// Saved selections, restored only when offered by the agent.
    /// A saved model takes precedence over `model_preferences`.
    /// Other settings keep the agent's defaults when their saved value is unavailable.
    pub preferences: crate::AgentPreferences,

    /// An earlier session to continue, if the agent can load it.
    ///
    /// The agent replays its history before the session opens. When the agent cannot load
    /// sessions, or no longer has this one, a new session starts instead.
    pub session_to_load: Option<SessionId>,
}
