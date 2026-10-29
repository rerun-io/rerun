//! Resolving [`AgentSettings`] into an agent harness to start.

use std::path::PathBuf;

use super::{AgentEntry, AgentHarnessConfig, AgentProfileId, McpStdioServer, find_executable};
use crate::settings::{AgentLaunchError, AgentSettings, DEFAULT_MODEL_PREFERENCES, SessionContext};

impl AgentSettings {
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
            preamble: context.agent_harness_preamble.clone(),
            model_preferences: DEFAULT_MODEL_PREFERENCES
                .iter()
                .map(|&model| model.to_owned())
                .collect(),
            preferred_model: self.preferred_models.get(&self.profile_id).cloned(),
            preferred_mode: self.preferred_mode.clone(),
        })
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

    use super::{parse_env_vars, take_leading_env_vars};
    use crate::{AgentProfileId, AgentSettings, SessionContext};

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
