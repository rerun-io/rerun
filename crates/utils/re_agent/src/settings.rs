use std::path::PathBuf;

use crate::connection::{LaunchConfig, McpStdioServer};
use crate::profiles::{AgentEntry, find_executable};

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
}

/// Everything needed to launch an agent. Serializable so the host app can persist it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AgentSettings {
    /// One of [`crate::AgentProfile::builtin`], or [`Self::CUSTOM_PROFILE_ID`].
    /// Empty means "pick an installed agent and start it without asking".
    pub profile_id: String,

    /// Used when [`Self::profile_id`] is [`Self::CUSTOM_PROFILE_ID`].
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

    /// Set once the user has started an agent from the setup screen.
    /// From then on the panel starts the agent right away instead of showing setup again.
    pub setup_done: bool,
}

impl AgentSettings {
    pub const CUSTOM_PROFILE_ID: &'static str = "custom";

    /// Resolves these settings into something that can be spawned.
    pub fn launch_config(
        &self,
        agents: &[AgentEntry],
        context: &SessionContext,
    ) -> Result<LaunchConfig, String> {
        let (command, args, env) = if self.profile_id == Self::CUSTOM_PROFILE_ID {
            let mut parts = shell_words::split(&self.custom_command_line)
                .map_err(|err| format!("Invalid agent command line: {err}"))?;
            let env = take_leading_env_vars(&mut parts);
            if parts.is_empty() {
                return Err("Enter a command for the custom agent".to_owned());
            }
            let command = parts.remove(0);
            (command, parts, env)
        } else {
            let agent = agents
                .iter()
                .find(|agent| agent.profile.id == self.profile_id)
                .ok_or_else(|| format!("Unknown agent profile: {}", self.profile_id))?;
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

        let cwd = if self.cwd.trim().is_empty() {
            std::env::current_dir().map_err(|err| format!("Failed to get current dir: {err}"))?
        } else {
            PathBuf::from(self.cwd.trim())
        };
        if !cwd.is_dir() {
            return Err(format!(
                "Working directory does not exist: {}",
                cwd.display()
            ));
        }

        let mut mcp_servers = Vec::new();
        for server in self.mcp_servers.iter().filter(|s| s.enabled) {
            let mut parts = shell_words::split(&server.command_line).map_err(|err| {
                format!("Invalid MCP server command line for {}: {err}", server.name)
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

        Ok(LaunchConfig {
            command,
            args,
            env,
            cwd,
            additional_directories: context.additional_directories.clone(),
            mcp_servers,
            log_protocol: self.log_protocol,
            preamble: context.preamble.clone(),
        })
    }
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            profile_id: String::new(),
            custom_command_line: String::new(),
            cwd: String::new(),
            env_vars: String::new(),
            mcp_servers: Vec::new(),
            show_thoughts: true,
            log_protocol: false,
            setup_done: false,
        }
    }
}

fn is_env_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Parses one `NAME=value` per line. Blank lines and `#` comments are skipped.
fn parse_env_vars(text: &str) -> Result<Vec<(String, String)>, String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (name, value) = line
                .split_once('=')
                .filter(|(name, _)| is_env_name(name))
                .ok_or_else(|| {
                    format!("Invalid environment variable line: {line:?}. Expected NAME=value")
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
    use super::{parse_env_vars, take_leading_env_vars};

    #[test]
    fn env_var_lines() {
        assert_eq!(parse_env_vars(""), Ok(vec![]));
        assert_eq!(
            parse_env_vars(" A=1 \n\n# comment\nB_2==x=y\n"),
            Ok(vec![
                ("A".to_owned(), "1".to_owned()),
                ("B_2".to_owned(), "=x=y".to_owned()),
            ])
        );
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"));
        if let Ok(home) = home {
            assert_eq!(
                parse_env_vars("A=~/x\nB=~\nC=a~/b"),
                Ok(vec![
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
