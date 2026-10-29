use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use agent_client_protocol_schema::v1::{SessionConfigValueId, SessionModeId};

use crate::agent_harness_config::{AgentHarnessConfig, McpStdioServer};
use crate::model_api_backend::{ModelApiConfig, ModelApiProvider};
use crate::profiles::{AgentEntry, AgentProfileId, find_executable};

/// Fast models to select, best first, unless the user has picked a model for the profile.
///
/// The agent panel should feel fast, and flagship models are often too slow for that.
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
    /// Markdown sent to the agent together with its first prompt,
    /// e.g. "You are running inside the Rerun Viewer. …".
    pub preamble: Option<String>,

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

    /// Session mode to restore when an agent offers it.
    pub preferred_mode: Option<SessionModeId>,

    /// Exact model choice per agent profile; model IDs belong to the agent that offered them.
    pub preferred_models: BTreeMap<AgentProfileId, SessionConfigValueId>,

    /// Share prompts and responses with Rerun after redacting sensitive text.
    pub share_redacted_prompts: bool,

    /// Set once the user has started an agent from the setup screen.
    /// From then on the panel starts the agent right away instead of showing setup again.
    pub setup_done: bool,
}

/// Why [`AgentSettings`] could not be resolved into an agent to start.
#[derive(Debug, thiserror::Error)]
pub enum AgentLaunchError {
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

    /// The environment variable that holds the provider's API key is unset or blank.
    #[error(
        "Set {} in the environment the viewer starts from, then restart it",
        .provider.api_key_env_var()
    )]
    MissingApiKey { provider: ModelApiProvider },

    /// The working directory does not exist.
    #[error("Working directory does not exist: {}", .cwd.display())]
    MissingWorkingDirectory { cwd: PathBuf },

    /// No installed agent has the selected profile.
    #[error("Unknown agent profile: {profile_id}")]
    UnknownProfile { profile_id: AgentProfileId },
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
            return self
                .agent_harness_config(agents, context)
                .map(AgentLaunch::AgentHarness);
        };
        let api_key = provider
            .api_key_from_env()
            .ok_or(AgentLaunchError::MissingApiKey { provider })?;
        let model = self.preferred_models.get(&self.profile_id).map_or_else(
            || provider.default_model().to_owned(),
            |model| model.0.to_string(),
        );
        Ok(AgentLaunch::ModelApi(ModelApiConfig {
            provider,
            api_key,
            model,
            system_prompt: context.preamble.clone(),
            toolbox: None,
        }))
    }

    /// Resolves these settings into an agent subprocess that can be spawned.
    pub fn agent_harness_config(
        &self,
        agents: &[AgentEntry],
        context: &SessionContext,
    ) -> Result<AgentHarnessConfig, AgentLaunchError> {
        let (command, args, env) = if self.profile_id == AgentProfileId::CUSTOM {
            let mut parts = shell_words::split(&self.custom_command_line)
                .map_err(|err| AgentLaunchError::InvalidCommandLine { err })?;
            let env = take_leading_env_vars(&mut parts);
            if parts.is_empty() {
                return Err(AgentLaunchError::EmptyCommandLine);
            }
            let command = parts.remove(0);
            (command, parts, env)
        } else {
            let agent = agents
                .iter()
                .find(|agent| agent.profile.id == self.profile_id)
                .ok_or_else(|| AgentLaunchError::UnknownProfile {
                    profile_id: self.profile_id.clone(),
                })?;
            let profile = &agent.profile;
            (
                profile.command.clone(),
                profile.args.clone(),
                profile.env.clone(),
            )
        };

        let mut env = env;
        env.extend(parse_env_vars(&self.env_vars)?);

        // Resolve to an absolute path so that agents installed outside the app's PATH still work.
        let command = find_executable(&command).unwrap_or_else(|| PathBuf::from(&command));

        let cwd = if !self.cwd.trim().is_empty() {
            PathBuf::from(self.cwd.trim())
        } else if let Some(cwd) = context.default_cwd.clone() {
            cwd
        } else {
            std::env::current_dir().map_err(|err| AgentLaunchError::CurrentDir { err })?
        };
        if !cwd.is_dir() {
            return Err(AgentLaunchError::MissingWorkingDirectory { cwd });
        }

        let mut mcp_servers = Vec::new();
        for server in self.mcp_servers.iter().filter(|s| s.enabled) {
            let mut parts = shell_words::split(&server.command_line).map_err(|err| {
                AgentLaunchError::InvalidMcpCommandLine {
                    server_name: server.name.clone(),
                    err,
                }
            })?;
            if parts.is_empty() {
                continue;
            }
            let command = parts.remove(0);
            let command = find_executable(&command).unwrap_or_else(|| PathBuf::from(&command));
            mcp_servers.push(McpStdioServer {
                name: server.name.clone(),
                command,
                args: parts,
            });
        }

        Ok(AgentHarnessConfig {
            command,
            args,
            env,
            cwd,
            additional_directories: context.additional_directories.clone(),
            off_limits_directories: context.off_limits_directories.clone(),
            mcp_servers,
            log_protocol: self.log_protocol,
            preamble: context.preamble.clone(),
            model_preferences: DEFAULT_MODEL_PREFERENCES
                .iter()
                .map(|&model| model.to_owned())
                .collect(),
            preferred_model: self.preferred_models.get(&self.profile_id).cloned(),
            preferred_mode: self.preferred_mode.clone(),
        })
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
            preferred_mode: None,
            preferred_models: BTreeMap::new(),
            share_redacted_prompts: false,
            setup_done: false,
        }
    }
}

fn is_env_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Parses one `NAME=value` per line. Blank lines and `#` comments are skipped.
fn parse_env_vars(text: &str) -> Result<Vec<(String, String)>, AgentLaunchError> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (name, value) = line
                .split_once('=')
                .filter(|(name, _)| is_env_name(name))
                .ok_or_else(|| AgentLaunchError::InvalidEnvVarLine {
                    line: line.to_owned(),
                })?;
            Ok((name.to_owned(), expand_home(value)))
        })
        .collect()
}

/// Expands a leading `~` to the home directory, since the value is not going through a shell.
fn expand_home(value: &str) -> String {
    let home = || std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"));
    if value == "~" {
        home().unwrap_or_else(|_| value.to_owned())
    } else if let Some(rest) = value.strip_prefix("~/") {
        match home() {
            Ok(home) => format!("{home}/{rest}"),
            Err(_) => value.to_owned(),
        }
    } else {
        value.to_owned()
    }
}

/// Splits off leading `NAME=value` words, the way a shell treats `FOO=1 cmd --flag`.
fn take_leading_env_vars(parts: &mut Vec<String>) -> Vec<(String, String)> {
    let mut env = Vec::new();
    while let Some((name, value)) = parts.first().and_then(|part| part.split_once('=')) {
        if !is_env_name(name) {
            break;
        }
        env.push((name.to_owned(), value.to_owned()));
        parts.remove(0);
    }
    env
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use agent_client_protocol_schema::v1::SessionConfigValueId;

    use super::{
        AgentProfileId, AgentSettings, SessionContext, parse_env_vars, take_leading_env_vars,
    };

    #[test]
    fn the_host_picks_the_working_directory_only_as_a_default() {
        let cwd = |setting: &str, default_cwd: Option<&str>| {
            let settings = AgentSettings {
                profile_id: AgentProfileId::CUSTOM,
                custom_command_line: "agent".to_owned(),
                cwd: setting.to_owned(),
                ..Default::default()
            };
            let context = SessionContext {
                default_cwd: default_cwd.map(PathBuf::from),
                ..Default::default()
            };
            settings
                .agent_harness_config(&[], &context)
                .map(|config| config.cwd)
                .ok()
        };

        let here = std::env::current_dir().expect("a current directory");
        let parent = here.parent().expect("a parent directory");
        let parent_str = parent.to_str().expect("a printable parent directory");

        // The host's default only applies when the user has not chosen a directory.
        assert_eq!(cwd("", Some(parent_str)).as_deref(), Some(parent));
        assert_eq!(cwd(parent_str, None).as_deref(), Some(parent));
        assert_eq!(cwd("", None).as_deref(), Some(here.as_path()));

        // A user setting of only whitespace is no setting at all.
        assert_eq!(cwd("  ", Some(parent_str)).as_deref(), Some(parent));

        // Neither source may name a directory that does not exist.
        assert!(cwd(&here.join("nope").display().to_string(), None).is_none());
        assert!(cwd("", Some(&here.join("nope").display().to_string())).is_none());
    }

    #[test]
    fn old_settings_use_new_defaults() {
        let Ok(settings) = serde_json::from_str::<AgentSettings>("{}") else {
            panic!("empty settings should deserialize");
        };
        assert!(!settings.share_redacted_prompts);
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

    #[test]
    fn env_var_lines() {
        assert_eq!(parse_env_vars("").ok(), Some(vec![]));
        assert_eq!(
            parse_env_vars(" A=1 \n\n# comment\nB_2==x=y\n").ok(),
            Some(vec![
                ("A".to_owned(), "1".to_owned()),
                ("B_2".to_owned(), "=x=y".to_owned()),
            ])
        );
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"));
        if let Ok(home) = home {
            assert_eq!(
                parse_env_vars("A=~/x\nB=~\nC=a~/b").ok(),
                Some(vec![
                    ("A".to_owned(), format!("{home}/x")),
                    ("B".to_owned(), home.clone()),
                    ("C".to_owned(), "a~/b".to_owned()),
                ])
            );
        }
        assert!(parse_env_vars("NOEQUALS").is_err());
        assert!(parse_env_vars("=1").is_err());
        assert!(parse_env_vars("A-B=1").is_err());
    }

    #[test]
    fn leading_env_vars() {
        let split = |line: &str| {
            let mut parts: Vec<String> = line.split(' ').map(str::to_owned).collect();
            let env = take_leading_env_vars(&mut parts);
            (env, parts)
        };

        assert_eq!(
            split("cmd a=b"),
            (vec![], vec!["cmd".to_owned(), "a=b".to_owned()])
        );
        assert_eq!(
            split("A=1 B_2= cmd"),
            (
                vec![
                    ("A".to_owned(), "1".to_owned()),
                    ("B_2".to_owned(), String::new())
                ],
                vec!["cmd".to_owned()]
            )
        );
        assert_eq!(
            split("=1 cmd"),
            (vec![], vec!["=1".to_owned(), "cmd".to_owned()])
        );
        assert_eq!(
            split("a-b=1 cmd"),
            (vec![], vec!["a-b=1".to_owned(), "cmd".to_owned()])
        );
    }
}
