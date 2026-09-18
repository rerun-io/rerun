//! The agent panel: a chat with a coding agent that runs next to the viewer and drives it
//! through `rerun viewer-mcp`.

use std::path::Path;

use re_agent_ui::{AgentPanel, AgentSettings, HostButton, McpServerConfig, SessionContext};

mod preamble;
mod self_improve;

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

    /// Empty working directory handed to the agent in our own development builds, kept alive
    /// for as long as the panel is, since dropping it deletes the directory.
    ///
    /// One directory for the whole panel, so every conversation in it shares a working
    /// directory — as they already do for a user who spawns several agents in one viewer.
    scratch_cwd: Option<tempfile::TempDir>,

    /// The Rerun checkout the viewer was started from, in our own development builds.
    /// Hidden from the default chat agent, but used for self-improvement.
    rerun_workspace: Option<std::path::PathBuf>,

    /// Where the session transcripts handed to self-improvement conversations are written.
    /// Kept alive for as long as the panel is, since dropping it deletes the directory.
    transcript_dir: Option<tempfile::TempDir>,

    /// Counts the self-improvement conversations started, so their transcripts get separate files.
    self_improvements_started: usize,

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
        is_in_rerun_workspace: bool,
    ) {
        let was_open = *open;
        if *open && self.panel.is_none() {
            self.initialize(
                settings.clone(),
                viewer_endpoint,
                cache_dir,
                is_in_rerun_workspace,
            );
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

        if self
            .panel
            .as_ref()
            .is_some_and(AgentPanel::host_button_clicked)
        {
            self.start_self_improvement();
        }

        self.record_finished_turns();
    }

    /// Open a conversation that reviews the active session and improves what let it down.
    ///
    /// The reviewing agent works in the Rerun checkout, which the reviewed session was kept out
    /// of, and reads the session as a transcript dumped to a file: the file is evidence the
    /// reviewed agent cannot talk its way around, and it carries the tool calls and the timings
    /// that the smells show up in.
    fn start_self_improvement(&mut self) {
        let Self {
            panel,
            rerun_workspace,
            transcript_dir,
            self_improvements_started,
            ..
        } = self;
        let (Some(panel), Some(workspace)) = (panel.as_mut(), rerun_workspace.as_ref()) else {
            return;
        };

        // The review preamble tells the second agent that the first one "just finished", and a
        // dump taken mid-turn is missing the answer and the tool results still to come — which
        // it can never gain, since the file is written once.
        if panel.turn_in_progress() {
            re_log::warn!("Wait for the agent to finish its turn before reviewing the session");
            return;
        }

        let Some(transcript) = panel.transcript().map(re_agent_ui::Transcript::to_markdown) else {
            return;
        };
        if transcript.trim().is_empty() {
            re_log::warn!("There is no agent session to review yet");
            return;
        }

        let dir = match transcript_dir {
            Some(dir) => dir,
            None => match tempfile::TempDir::with_prefix("rerun-agent-review-") {
                Ok(dir) => transcript_dir.insert(dir),
                Err(err) => {
                    re_log::warn!("Failed to create a directory for the session transcript: {err}");
                    return;
                }
            },
        };
        let transcript = match self_improve::write_transcript(
            dir.path(),
            *self_improvements_started,
            &transcript,
        ) {
            Ok(path) => path,
            Err(err) => {
                re_log::warn!("Failed to write the session transcript: {err}");
                return;
            }
        };
        *self_improvements_started += 1;

        panel.open_conversation(
            self_improve::session_context(workspace, dir.path()),
            self_improve::opening_prompt(&transcript),
        );
    }

    /// Initialize the inner panel.
    ///
    /// `viewer_endpoint` is the gRPC address of this viewer, which `rerun viewer-mcp` connects to.
    /// `cache_dir` is where the skills are unpacked for the agent to read.
    /// `is_in_rerun_workspace` marks one of our own development builds; see [`Self::user_like_cwd`].
    fn initialize(
        &mut self,
        mut settings: AgentSettings,
        viewer_endpoint: Option<&str>,
        cache_dir: Option<&Path>,
        is_in_rerun_workspace: bool,
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

        // Only applied when the user has not set a working directory of their own.
        let rerun_workspace = is_in_rerun_workspace
            .then(|| self.user_like_cwd())
            .flatten();
        self.rerun_workspace.clone_from(&rerun_workspace);
        let off_limits_directories: Vec<_> = rerun_workspace.iter().cloned().collect();

        let mut panel = AgentPanel::new(settings);
        panel.set_host_button(rerun_workspace.is_some().then(|| {
            HostButton {
                label: "Self-improve".to_owned(),
                tooltip: "Rerun development builds only, never shown in a release build.\n\n\
                      Opens a second agent in the Rerun checkout that reviews this session, \
                      finds what the viewer's MCP tools, skills, docs, and instructions made \
                      hard, and files the fixes."
                    .into(),
            }
        }));
        panel.set_context(SessionContext {
            preamble: Some(
                preamble::Preamble {
                    viewer_endpoint,
                    agent_dir: agent_dir.as_deref(),
                    off_limits_directories: &off_limits_directories,
                }
                .text(),
            ),
            additional_directories: agent_dir.iter().cloned().collect(),
            default_cwd: self
                .scratch_cwd
                .as_ref()
                .map(|scratch| scratch.path().to_owned()),
            off_limits_directories,
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

    /// Point the agent at an empty scratch directory instead of the Rerun checkout, and return
    /// the checkout it was spared.
    ///
    /// A released viewer runs in the user's own directory, so the agent sees the user's project
    /// and not ours. Started from the Rerun workspace, the same code hands the agent our source
    /// tree, and it starts answering from code that a user's agent cannot read. This keeps our
    /// development builds honest.
    ///
    /// Returns `None` when the viewer was not started from a Rerun checkout — a development
    /// binary run from somewhere else has none of our source to hide, and the directory it did
    /// start in belongs to whoever is running it — or when there is no scratch directory to use
    /// instead.
    fn user_like_cwd(&mut self) -> Option<std::path::PathBuf> {
        let workspace = self_improve::rerun_checkout(&std::env::current_dir().ok()?)?;

        if self.scratch_cwd.is_none() {
            match tempfile::TempDir::with_prefix("rerun-agent-") {
                Ok(dir) => self.scratch_cwd = Some(dir),
                Err(err) => {
                    re_log::warn_once!(
                        "Failed to create a scratch directory for the agent, \
                         so it can read the Rerun source tree: {err}"
                    );
                    return None;
                }
            }
        }
        Some(workspace)
    }
}

/// `rerun viewer-mcp`, pointed at this viewer.
///
/// Prefers the running executable, so that the agent drives the viewer it is embedded in rather
/// than some other version that happens to be installed.
///
/// The name check keeps this honest: `re_viewer` is a library, so the running executable may be a
/// custom viewer or one of our examples, and only the `rerun` binary has a `viewer-mcp` subcommand.
/// It is case-insensitive because the macOS bundle names the executable `Rerun`.
fn rerun_mcp_server(viewer_endpoint: Option<&str>) -> Option<McpServerConfig> {
    let current_exe = std::env::current_exe().ok().filter(|exe| {
        exe.file_stem()
            .is_some_and(|stem| stem.to_string_lossy().to_lowercase().starts_with("rerun"))
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
    fn a_development_build_hides_the_rerun_checkout() {
        let mut panel = ViewerAgentPanel::default();

        // The checkout, not the directory the test happens to run in: `cargo` starts a test in
        // its own crate directory, well inside the checkout.
        let workspace = panel.user_like_cwd().expect("a working directory to hide");
        assert!(
            std::env::current_dir()
                .expect("cwd")
                .starts_with(&workspace),
            "{workspace:?} must contain the directory the viewer was started from"
        );
        assert!(workspace.join("crates/top/re_viewer/Cargo.toml").is_file());

        let scratch = panel
            .scratch_cwd
            .as_ref()
            .expect("a scratch directory")
            .path()
            .to_owned();
        assert!(scratch.is_dir());
        assert!(!workspace.starts_with(&scratch));
        assert_eq!(
            std::fs::read_dir(&scratch).expect("read_dir").count(),
            0,
            "the agent must not start in a directory with anything to read"
        );

        // Asking twice keeps the same directory, so reopening the panel does not litter.
        let again = panel.user_like_cwd().expect("a working directory to hide");
        assert_eq!(again, workspace);
        assert_eq!(
            panel
                .scratch_cwd
                .as_ref()
                .expect("a scratch directory")
                .path(),
            scratch
        );
    }

    #[test]
    fn only_a_development_build_is_told_to_hold_back() {
        let checkout = std::path::PathBuf::from("/home/someone/rerun");
        let directories = [checkout.clone()];
        let text = preamble::Preamble {
            off_limits_directories: &directories,
            ..Default::default()
        }
        .text();
        assert!(text.contains("Do NOT read the source code of Rerun"));
        // The request names the directory the breach report is measured against:
        assert!(text.contains(&checkout.display().to_string()));

        assert!(
            !preamble::Preamble::default()
                .text()
                .contains("Do NOT read the source code of Rerun")
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
