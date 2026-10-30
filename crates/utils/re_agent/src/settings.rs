use std::collections::BTreeMap;
#[cfg(agent_harness_backend)]
use std::path::Path;
use std::path::PathBuf;

use agent_client_protocol_schema::v1::{SessionConfigValueId, SessionModeId};

use crate::agent_harness::{AgentEntry, AgentHarnessConfig, AgentProfileId};
use crate::model_api_backend::{ApiKey, HostedTools, ModelApiConfig, ModelApiProvider};

/// Fast models to select, best first, unless the user has picked a model for the profile.
///
/// The agent panel should feel fast, and flagship models are often too slow for that.
#[cfg(agent_harness_backend)]
pub const DEFAULT_MODEL_PREFERENCES: &[&str] = &[
    "sonnet", // Anthropic: Claude
    "flash",  // Google: Gemini
    "luna",   // OpenAI: Codex
    "mini",   // OpenAI: older GPT models
];

/// An MCP server the agent should connect to, spawned by the agent over stdio.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct McpServerConfig {
    /// The name the agent sees the server under. Tools show up as `mcp__<name>__<tool>`.
    pub name: String,

    /// Full command line, e.g. `rerun viewer-mcp`.
    pub command_line: String,

    /// Disabled servers stay in the list but are not handed to the agent.
    pub enabled: bool,
}

impl McpServerConfig {
    /// Quotes `command` and `args` so that [`Self::command_line`] splits back into them.
    #[cfg(agent_harness_backend)]
    pub fn new(name: impl Into<String>, command: &Path, args: &[impl AsRef<str>]) -> Self {
        let command = command.to_string_lossy();
        let words = std::iter::chain(
            std::iter::once(command.as_ref()),
            args.iter().map(AsRef::as_ref),
        );
        Self {
            name: name.into(),
            command_line: shell_words::join(words),
            enabled: true,
        }
    }
}

/// What the host app wants every new session to know, on top of the user's settings.
///
/// Added to the agent's own context, not instead of it: the agent still loads its default
/// system prompt, `CLAUDE.md`/`AGENTS.md`, skills, and configured MCP servers from the working
/// directory and the user's home, as it would in a terminal.
///
/// Not persisted: it describes the host, not the user's choices.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionContext {
    /// Markdown sent to an agent harness together with its first prompt,
    /// e.g. "You are running inside the Rerun Viewer. …".
    ///
    /// A model-API agent never gets it: see [`Self::model_api_preamble`].
    pub agent_harness_preamble: Option<String>,

    /// System prompt for a model-API agent, sent with every request.
    ///
    /// Separate from [`Self::agent_harness_preamble`], since the two have different tools: a
    /// harness reaches the viewer over MCP and can read files, a model-API agent cannot.
    pub model_api_preamble: Option<String>,

    /// Directories the agent may read besides the working directory.
    /// Claude Code loads skills from `.claude/skills/` inside these.
    pub additional_directories: Vec<PathBuf>,

    /// Working directory for sessions where the user has not chosen one.
    /// Unset means the directory the host process was started in.
    pub default_cwd: Option<PathBuf>,

    /// Directories the agent is asked, in its preamble, to stay out of.
    ///
    /// Nothing enforces this: the agent can still reach them, and the user is still asked to
    /// approve. Paths that land inside them are counted in
    /// [`crate::TurnReport::off_limits_paths`] so that the host can tell whether the agent
    /// honored the request.
    pub off_limits_directories: Vec<PathBuf>,
}

/// Everything needed to launch an agent. Serializable so the host app can persist it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AgentSettings {
    /// One of [`crate::AgentProfile::builtin`], or [`AgentProfileId::CUSTOM`].
    /// Empty means "pick an installed agent and start it without asking".
    pub profile_id: AgentProfileId,

    /// Used when [`Self::profile_id`] is [`AgentProfileId::CUSTOM`].
    pub custom_command_line: String,

    /// Working directory for the agent session. Empty means the current directory.
    pub cwd: String,

    /// Environment variables for the agent process, one `NAME=value` per line,
    /// e.g. `CLAUDE_CONFIG_DIR=~/.claude-work` to pick a profile.
    pub env_vars: String,

    /// MCP servers handed to the agent when a session starts.
    pub mcp_servers: Vec<McpServerConfig>,

    /// Show the agent's reasoning in the transcript, not just its answers.
    pub show_thoughts: bool,

    /// Show every JSON-RPC line exchanged with the agent in the log view.
    pub log_protocol: bool,

    /// The tools a model-API agent's provider runs for it on its own servers.
    pub model_api_hosted_tools: HostedTools,

    /// Session mode to restore when an agent offers it.
    pub preferred_mode: Option<SessionModeId>,

    /// Exact model choice per agent profile; model IDs belong to the agent that offered them.
    pub preferred_models: BTreeMap<AgentProfileId, SessionConfigValueId>,

    /// Share prompts and responses with Rerun after redacting sensitive text.
    pub share_redacted_prompts: bool,

    /// Set once the user has started an agent from the setup screen.
    /// From then on the panel starts the agent right away instead of showing setup again.
    pub setup_done: bool,

    /// API keys as the user typed them, by model-API profile.
    ///
    /// Only the web viewer uses these, since on native we want to implement a more secure storage
    /// first. Web stores all settings in the browser's local storage.
    ///
    /// The desktop viewer reads keys from the environment only, and never stores them.
    #[cfg(target_arch = "wasm32")]
    pub api_keys: BTreeMap<AgentProfileId, String>,
}

/// Why [`AgentSettings`] could not be resolved into an agent to start.
#[derive(Debug, thiserror::Error)]
pub enum AgentLaunchError {
    /// The profile runs an agent on the user's machine, which this build cannot do.
    #[error("{profile_id} runs on your machine, which this viewer cannot do. Pick a model API")]
    AgentHarnessUnavailable { profile_id: AgentProfileId },

    /// The user has not entered an API key for the provider. Only on the web, where keys are
    /// typed in.
    #[error("Enter your {} API key", .provider.name())]
    ApiKeyNotEntered { provider: ModelApiProvider },

    /// The provider's API key is unset or blank in the environment. Only on the desktop, where
    /// keys come from the environment.
    #[error(
        "Set {} in the environment the viewer starts from, then restart it",
        .provider.api_key_env_var()
    )]
    ApiKeyNotInEnvironment { provider: ModelApiProvider },

    /// The current directory, used when no working directory is set, could not be read.
    #[error("Failed to get current dir: {err}")]
    CurrentDir { err: std::io::Error },

    /// The custom agent's command line has no command.
    #[error("Enter a command for the custom agent")]
    EmptyCommandLine,

    /// The custom agent's command line could not be split into words.
    #[error("Invalid agent command line: {err}")]
    InvalidCommandLine { err: shell_words::ParseError },

    /// A line of the environment variables is not `NAME=value`.
    #[error("Invalid environment variable line: {line:?}. Expected NAME=value")]
    InvalidEnvVarLine { line: String },

    /// The command line of an enabled MCP server could not be split into words.
    #[error("Invalid MCP server command line for {server_name}: {err}")]
    InvalidMcpCommandLine {
        server_name: String,
        err: shell_words::ParseError,
    },

    /// The working directory does not exist.
    #[error("Working directory does not exist: {}", .cwd.display())]
    MissingWorkingDirectory { cwd: PathBuf },

    /// No installed agent has the selected profile.
    #[error("Unknown agent profile: {profile_id}")]
    UnknownProfile { profile_id: AgentProfileId },
}

impl AgentLaunchError {
    /// The error for a missing key for `provider`, from where [`AgentSettings::api_key`] looks.
    pub fn missing_api_key(provider: ModelApiProvider) -> Self {
        if crate::API_KEYS_IN_SETTINGS {
            Self::ApiKeyNotEntered { provider }
        } else {
            Self::ApiKeyNotInEnvironment { provider }
        }
    }
}

/// What to start: an agent subprocess, or an agent loop that calls a provider's API directly.
#[derive(Debug)]
pub enum AgentLaunch {
    /// Spawn an agent (Claude Code, Codex, …) and talk to it over the Agent Client Protocol.
    AgentHarness(AgentHarnessConfig),

    /// Run the agent loop ourselves, calling the model provider's API directly.
    ModelApi(ModelApiConfig),
}

impl AgentSettings {
    /// Resolves these settings into an agent to start.
    pub fn launch(
        &self,
        agents: &[AgentEntry],
        context: &SessionContext,
    ) -> Result<AgentLaunch, AgentLaunchError> {
        let Some(provider) = self.profile_id.model_api_provider() else {
            return cfg_select! {
                agent_harness_backend => self
                    .agent_harness_config(agents, context)
                    .map(AgentLaunch::AgentHarness),
                _ => {{
                    _ = agents;
                    Err(AgentLaunchError::AgentHarnessUnavailable {
                        profile_id: self.profile_id.clone(),
                    })
                }}
            };
        };
        let api_key = self
            .api_key(provider)
            .ok_or_else(|| AgentLaunchError::missing_api_key(provider))?;
        let model = self.preferred_models.get(&self.profile_id).map_or_else(
            || provider.default_model().to_owned(),
            |model| model.0.to_string(),
        );
        Ok(AgentLaunch::ModelApi(ModelApiConfig {
            provider,
            api_key,
            model,
            system_prompt: context.model_api_preamble.clone(),
            hosted_tools: self.model_api_hosted_tools,
            toolbox: None,
        }))
    }

    /// The key for `provider`: from the settings or from the environment, as
    /// [`crate::API_KEYS_IN_SETTINGS`] says.
    pub fn api_key(&self, provider: ModelApiProvider) -> Option<ApiKey> {
        cfg_select! {
            target_arch = "wasm32" => self
                .api_keys
                .get(&provider.profile_id())
                .and_then(|text| ApiKey::new(text)),
            _ => {{
                _ = self;
                provider.api_key_from_env()
            }}
        }
    }
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            profile_id: AgentProfileId::default(),
            custom_command_line: String::new(),
            cwd: String::new(),
            env_vars: String::new(),
            mcp_servers: Vec::new(),
            show_thoughts: true,
            log_protocol: false,
            model_api_hosted_tools: HostedTools::default(),
            preferred_mode: None,
            preferred_models: BTreeMap::new(),
            share_redacted_prompts: false,
            setup_done: false,
            #[cfg(target_arch = "wasm32")]
            api_keys: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use agent_client_protocol_schema::v1::SessionConfigValueId;

    use super::{AgentProfileId, AgentSettings, HostedTools};

    #[test]
    fn old_settings_use_new_defaults() {
        let Ok(settings) = serde_json::from_str::<AgentSettings>("{}") else {
            panic!("empty settings should deserialize");
        };
        assert!(!settings.share_redacted_prompts);
        assert_eq!(settings.model_api_hosted_tools, HostedTools::default());
        assert_eq!(settings.preferred_mode, None);
        assert!(settings.preferred_models.is_empty());
    }

    #[test]
    fn profile_ids_are_stored_as_plain_strings() {
        let mut settings = AgentSettings {
            profile_id: AgentProfileId::new("codex"),
            ..Default::default()
        };
        settings.preferred_models.insert(
            AgentProfileId::new("claude"),
            SessionConfigValueId::new("sonnet"),
        );
        let Ok(json) = serde_json::to_value(&settings) else {
            panic!("settings should serialize");
        };
        assert_eq!(json["profile_id"], "codex");
        assert_eq!(json["preferred_models"]["claude"], "sonnet");
        assert_eq!(
            serde_json::from_value::<AgentSettings>(json).ok(),
            Some(settings)
        );
    }
}
