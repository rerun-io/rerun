//! The agent panel: a chat with a coding agent that runs next to the viewer and drives it
//! through `rerun viewer-mcp`.

use std::path::Path;

use re_agent_ui::{AgentPanel, AgentSettings, McpServerConfig, SessionContext};

mod preamble;

/// The MCP server name the agent sees the viewer under. Tools show up as `mcp__rerun__<tool>`.
const MCP_SERVER_NAME: &str = "rerun";

/// Sub-directory of the viewer cache that the agent may read. Holds the skills and docs.
const AGENT_CACHE_SUBDIR: &str = "agent";

/// Where the skills are unpacked inside the agent directory.
const SKILLS_SUBDIR: &str = "agent_context/skills";

/// Where the documentation and snippets are unpacked inside the agent directory.
const DOCS_SUBDIR: &str = "agent_context/docs";

/// The skills and docs from the repository, embedded by `build.rs`.
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/agent_files.rs"));
}

/// The agent chat, configured for the viewer it lives in.
#[derive(Default)]
pub struct ViewerAgentPanel {
    /// Created the first time the panel opens, since initialization probes for installed agents.
    panel: Option<AgentPanel>,

    /// Set when the panel is opened, so the prompt input takes focus once there is one.
    focus_input: bool,

    #[cfg(feature = "analytics")]
    analytics: crate::agent_analytics::TurnAnalytics,
}

impl ViewerAgentPanel {
    /// Give the prompt input keyboard focus once there is a panel to give it to.
    pub fn request_input_focus(&mut self) {
        self.focus_input = true;
    }

    /// Show the panel and persist changed settings.
    ///
    /// `open` lives in `AppState` so that it survives a restart.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        open: &mut bool,
        settings: &mut AgentSettings,
        viewer_endpoint: Option<&str>,
        cache_dir: Option<&Path>,
    ) {
        let was_open = *open;
        if *open && self.panel.is_none() {
            self.initialize(settings.clone(), viewer_endpoint, cache_dir);
        }

        let Self {
            panel, focus_input, ..
        } = self;
        if let Some(panel) = panel.as_mut() {
            // A collapsed panel never runs its `ui`, and its agents would otherwise
            // sit on their messages until the user opens it again.
            panel.poll_events();

            if *focus_input {
                panel.request_input_focus();
                *focus_input = false;
            }
        }
        egui::Panel::right("agent_panel")
            .default_size(re_agent_ui::RECOMMENDED_WIDTH)
            .resizable(true)
            .frame(egui::Frame {
                fill: ui.visuals().panel_fill,
                ..Default::default()
            })
            .show_collapsible(ui, open, |ui| {
                if let Some(panel) = panel {
                    panel.ui(ui);
                    settings.clone_from(panel.settings());
                }
            });

        // Dragging the collapsed panel open does not go through `toggle_agent_panel`:
        self.focus_input |= !was_open && *open;

        self.record_finished_turns();
    }

    /// Initialize the inner panel.
    ///
    /// `viewer_endpoint` is the gRPC address of this viewer, which `rerun viewer-mcp` connects to.
    /// `cache_dir` is where the skills are unpacked for the agent to read.
    fn initialize(
        &mut self,
        mut settings: AgentSettings,
        viewer_endpoint: Option<&str>,
        cache_dir: Option<&Path>,
    ) {
        re_tracing::profile_function!();

        // The viewer's port can change between runs, so the persisted entry is replaced.
        settings
            .mcp_servers
            .retain(|server| server.name != MCP_SERVER_NAME);
        if let Some(server) = rerun_mcp_server(viewer_endpoint) {
            settings.mcp_servers.insert(0, server);
        } else {
            re_log::warn_once!(
                "Could not find the `rerun` executable, so the agent cannot connect to the viewer"
            );
        }

        let agent_dir = cache_dir
            .filter(|_| !embedded::AGENT_FILES.is_empty())
            .and_then(|cache_dir| {
                let agent_dir = cache_dir.join(AGENT_CACHE_SUBDIR);
                // The directory itself has to exist before the agent is told about it;
                // filling it is thousands of small writes, so that happens off this thread.
                match std::fs::create_dir_all(&agent_dir) {
                    Ok(()) => {
                        install_agent_files_in_background(agent_dir.clone());
                        Some(agent_dir)
                    }
                    Err(err) => {
                        re_log::warn_once!("Failed to create {agent_dir:?}: {err}");
                        None
                    }
                }
            });

        let mut panel = AgentPanel::new(settings);
        panel.set_context(SessionContext {
            preamble: Some(preamble::text(viewer_endpoint, agent_dir.as_deref())),
            additional_directories: agent_dir.iter().cloned().collect(),
        });
        self.panel = Some(panel);
    }

    /// Record non-content usage for every turn and, when enabled, share its redacted text.
    fn record_finished_turns(&mut self) {
        let Some(panel) = &mut self.panel else {
            return;
        };
        let share_redacted_prompts = panel.settings().share_redacted_prompts;
        let turns = panel.take_finished_turns();
        cfg_select! {
            feature = "analytics" => {
                // Redaction spawns an agent session per turn, so it must not run when the
                // event it prepares would be dropped anyway.
                let share_redacted_prompts = share_redacted_prompts && re_analytics::is_enabled();
                let config = if share_redacted_prompts && !turns.is_empty() {
                    panel.launch_config(&SessionContext::default()).ok()
                } else {
                    None
                };
                for turn in turns {
                    crate::agent_analytics::record_usage(&turn);
                    if share_redacted_prompts {
                        self.analytics.queue(turn, config.clone());
                    }
                }
                if !share_redacted_prompts {
                    // The user may have just unticked the box: drop what is still queued.
                    self.analytics.discard_pending();
                }
            }
            _ => {
                let _ = share_redacted_prompts;
                drop(turns);
            }
        }
    }
}

/// `rerun viewer-mcp`, pointed at this viewer.
///
/// Prefers the running executable so that a dev build talks to itself,
/// and falls back to whatever `rerun` is on the `PATH`.
fn rerun_mcp_server(viewer_endpoint: Option<&str>) -> Option<McpServerConfig> {
    let current_exe = std::env::current_exe().ok().filter(|exe| {
        exe.file_stem()
            .is_some_and(|stem| stem.to_string_lossy().starts_with("rerun"))
    });
    let rerun = current_exe.or_else(|| re_agent_ui::find_executable("rerun"))?;

    let mut args = vec!["viewer-mcp".to_owned()];
    if let Some(endpoint) = viewer_endpoint {
        args.push("--endpoint".to_owned());
        args.push(endpoint.to_owned());
    }
    Some(McpServerConfig::new(MCP_SERVER_NAME, &rerun, &args))
}

/// Unpacks the embedded skills and docs into `agent_dir` on a background thread.
///
/// The agent is told about the directory right away. The files land well before it has read a
/// prompt and gone looking for a skill, and an agent that looks too early simply finds nothing.
fn install_agent_files_in_background(agent_dir: std::path::PathBuf) {
    let install = move || {
        if let Err(err) = install_agent_files(&agent_dir) {
            re_log::warn_once!("Failed to unpack the agent skills and docs: {err}");
        }
    };
    if let Err(err) = std::thread::Builder::new()
        .name("agent-context-install".to_owned())
        .spawn(install)
    {
        re_log::warn_once!("Failed to start the agent context install thread: {err}");
    }
}

/// Writes the embedded skills and docs into `agent_dir`, replacing what was there.
fn install_agent_files(agent_dir: &Path) -> std::io::Result<()> {
    re_tracing::profile_function!();

    for subdir in [SKILLS_SUBDIR, DOCS_SUBDIR] {
        let dir = agent_dir.join(subdir);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
    }
    for (relative_path, contents) in embedded::AGENT_FILES {
        let path = agent_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, contents)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "agent_context")]
    fn skills_and_docs_are_embedded() {
        let has = |wanted: &str| {
            embedded::AGENT_FILES
                .iter()
                .any(|(path, _)| *path == wanted)
        };
        assert!(has("agent_context/skills/rerun-docs/SKILL.md"));
        assert!(has("agent_context/docs/snippets/INDEX.md"));
        assert!(has("agent_context/docs/content/index.md"));
        assert!(
            !embedded::AGENT_FILES.iter().any(|(path, _)| {
                Path::new(path)
                    .extension()
                    .is_some_and(|ext| ext == "rrd" || ext == "png")
            }),
            "binary assets must not be embedded"
        );
    }

    #[test]
    fn install_writes_every_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        install_agent_files(dir.path()).expect("install");
        for (relative_path, contents) in embedded::AGENT_FILES {
            let path = dir.path().join(relative_path);
            assert_eq!(std::fs::read(&path).expect("read").as_slice(), *contents);
        }
    }
}
