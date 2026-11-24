//! Resolving [`AgentSettings`] into an agent harness to start.

use std::path::PathBuf;

use super::{AgentEntry, AgentHarnessConfig, AgentProfileId, McpStdioServer, find_executable};
use crate::settings::{AgentLaunchError, AgentSettings, SessionContext};

impl AgentSettings {
    /// Resolves these settings into an agent subprocess that can be spawned.
    pub fn agent_harness_config(
        &self,
        agents: &[AgentEntry],
        context: &SessionContext,
    ) -> Result<AgentHarnessConfig, AgentLaunchError> {
        let (command, args, profile_env, mut preferences) =
            if self.profile_id == AgentProfileId::CUSTOM {
                let mut parts = shell_words::split(&self.custom_command_line)
                    .map_err(|err| AgentLaunchError::InvalidCommandLine { err })?;
                let env = take_leading_env_vars(&mut parts);
                if parts.is_empty() {
                    return Err(AgentLaunchError::EmptyCommandLine);
                }
                let command = parts.remove(0);
                (command, parts, env, crate::AgentPreferences::default())
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
                    profile.default_config.clone(),
                )
            };

        // Resolve to an absolute path so that agents installed outside the app's PATH still work.
        let command = find_executable(&command).unwrap_or_else(|| PathBuf::from(&command));

        let mut env: Vec<(String, String)> = path_with_dir_of(&command).into_iter().collect();
        env.extend(profile_env);
        env.extend(parse_env_vars(&self.env_vars)?);

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

        let mut mcp_servers: Vec<McpStdioServer> = context
            .rerun_mcp_server
            .iter()
            .filter(|_| self.rerun_mcp_enabled)
            .map(|server| {
                let mut server = server.clone();
                match self.catalog_access {
                    re_agent_tools::CatalogAccess::ReadOnly => {
                        server.args.push("--catalog-read-only".to_owned());
                    }
                    re_agent_tools::CatalogAccess::ReadWrite => {}
                }
                server
            })
            .collect();
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

        if let Some(saved) = self.preferences(&self.profile_id) {
            preferences
                .config_options
                .extend(saved.config_options.clone());
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
            model_preselect: context.model_preselect,
            preferences,
            session_to_load: None,
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

fn path_with_dir_of(command: &std::path::Path) -> Option<(String, String)> {
    let dir = command.parent().filter(|dir| dir.is_absolute())?;
    let current = std::env::var_os("PATH").unwrap_or_default();
    let dirs = std::iter::chain([dir.to_path_buf()], std::env::split_paths(&current));
    let path = std::env::join_paths(dirs).ok()?.into_string().ok()?;
    Some(("PATH".to_owned(), path))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{parse_env_vars, path_with_dir_of, take_leading_env_vars};
    use crate::{AgentProfileId, AgentSettings, SessionContext};

    #[test]
    fn the_agent_path_starts_with_the_directory_of_its_command() {
        let node = tempfile::tempdir().expect("a temporary directory");
        let dir = node.path();
        let (name, path) = path_with_dir_of(&dir.join("npx")).expect("an absolute command");
        assert_eq!(name, "PATH");
        assert_eq!(std::env::split_paths(&path).next().as_deref(), Some(dir));
        assert_eq!(path_with_dir_of(std::path::Path::new("npx")), None);
    }

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
    fn the_host_s_own_mcp_server_comes_first_unless_turned_off() {
        let rerun = crate::McpStdioServer {
            name: "rerun".to_owned(),
            command: PathBuf::from("/bin/rerun"),
            args: vec!["mcp".to_owned()],
        };
        let context = SessionContext {
            rerun_mcp_server: Some(rerun.clone()),
            ..Default::default()
        };
        let names = |rerun_mcp_enabled: bool| {
            let settings = AgentSettings {
                profile_id: AgentProfileId::CUSTOM,
                custom_command_line: "agent".to_owned(),
                mcp_servers: vec![crate::McpServerConfig::new(
                    "mine",
                    std::path::Path::new("/bin/mine"),
                    &[] as &[&str],
                )],
                rerun_mcp_enabled,
                ..Default::default()
            };
            settings
                .agent_harness_config(&[], &context)
                .expect("config")
                .mcp_servers
                .into_iter()
                .map(|server| server.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(true), ["rerun", "mine"]);
        assert_eq!(names(false), ["mine"]);
    }

    #[test]
    fn without_write_access_the_host_s_server_starts_read_only() {
        let context = SessionContext {
            rerun_mcp_server: Some(crate::McpStdioServer {
                name: "rerun".to_owned(),
                command: PathBuf::from("/bin/rerun"),
                args: vec!["mcp".to_owned()],
            }),
            ..Default::default()
        };
        let args = |catalog_access: re_agent_tools::CatalogAccess| {
            let settings = AgentSettings {
                profile_id: AgentProfileId::CUSTOM,
                custom_command_line: "agent".to_owned(),
                catalog_access,
                ..Default::default()
            };
            settings
                .agent_harness_config(&[], &context)
                .expect("config")
                .mcp_servers
                .remove(0)
                .args
        };
        assert_eq!(args(re_agent_tools::CatalogAccess::ReadWrite), ["mcp"]);
        assert_eq!(
            args(re_agent_tools::CatalogAccess::ReadOnly),
            ["mcp", "--catalog-read-only"]
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
