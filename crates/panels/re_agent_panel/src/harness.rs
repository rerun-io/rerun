//! The parts of the agent panel that only exist where agent harnesses (Claude Code, Codex, …)
//! can run: the `rerun viewer-mcp` server they drive the viewer through, the skills and docs we
//! unpack for them, and self-improvement.

use std::path::Path;

use re_agent::{AgentSettings, McpServerConfig, SessionContext};
use re_agent_ui::AgentPanel;
use re_async::AsyncRuntimeHandle;

use super::{DOCS_SUBDIR, SKILLS_SUBDIR, preamble, self_improve};

/// The MCP server name the agent sees the viewer under. Tools show up as `mcp__rerun__<tool>`.
const MCP_SERVER_NAME: &str = "rerun";

/// Sub-directory of the viewer cache that the agent may read. Holds the skills and docs.
const AGENT_CACHE_SUBDIR: &str = "agent";

/// The skills and docs from the repository, embedded by `build.rs`.
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/agent_files.rs"));
}

/// What the agent panel keeps for agent harnesses.
#[derive(Default)]
pub struct Harness {
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
}

impl Harness {
    /// A new agent panel that can also run agent harnesses, which drive this viewer through
    /// `rerun viewer-mcp` and read the skills and docs we unpack for them.
    ///
    /// `viewer_endpoint` is the gRPC address of this viewer, which `rerun viewer-mcp` connects to.
    /// `cache_dir` is where the skills are unpacked for the agent to read.
    /// `is_in_rerun_workspace` marks one of our own development builds; see [`Self::user_like_cwd`].
    pub fn new_panel(
        &mut self,
        mut settings: AgentSettings,
        async_runtime: AsyncRuntimeHandle,
        viewer_endpoint: Option<&str>,
        cache_dir: Option<&Path>,
        is_in_rerun_workspace: bool,
    ) -> AgentPanel {
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

        let mut panel = AgentPanel::new(settings, async_runtime);
        panel.set_self_improve_available(rerun_workspace.is_some());
        panel.set_context(SessionContext {
            agent_harness_preamble: Some(
                preamble::Preamble {
                    viewer_endpoint,
                    agent_dir: agent_dir.as_deref(),
                    off_limits_directories: &off_limits_directories,
                }
                .text(),
            ),
            model_api_preamble: Some(preamble::model_api_text()),
            additional_directories: agent_dir.iter().cloned().collect(),
            default_cwd: self
                .scratch_cwd
                .as_ref()
                .map(|scratch| scratch.path().to_owned()),
            off_limits_directories,
        });
        panel
    }

    /// Start a self-improvement conversation if the user asked for one in the panel's `ui`.
    pub fn handle_self_improve_request(&mut self, panel: &mut AgentPanel) {
        if panel.self_improve_requested() {
            self.start_self_improvement(panel);
        }
    }

    /// Open a conversation that reviews the active session and improves what let it down.
    ///
    /// The reviewing agent works in the Rerun checkout, which the reviewed session was kept out
    /// of, and reads the session as a transcript dumped to a file: the file is evidence the
    /// reviewed agent cannot talk its way around, and it carries the tool calls and the timings
    /// that the smells show up in.
    fn start_self_improvement(&mut self, panel: &mut AgentPanel) {
        let Self {
            rerun_workspace,
            transcript_dir,
            self_improvements_started,
            ..
        } = self;
        let Some(workspace) = rerun_workspace.as_ref() else {
            return;
        };

        // The review preamble tells the second agent that the first one "just finished", and a
        // dump taken mid-turn is missing the answer and the tool results still to come — which
        // it can never gain, since the file is written once.
        if panel.is_turn_in_progress() {
            re_log::warn!("Wait for the agent to finish its turn before reviewing the session");
            return;
        }

        let transcript = self_improve::session_dump(panel.session());
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
    let current_exe = std::env::current_exe();
    let rerun = match &current_exe {
        Ok(exe)
            if exe
                .file_stem()
                .is_some_and(|stem| stem.to_string_lossy().to_lowercase().starts_with("rerun")) =>
        {
            exe.clone()
        }
        _ => {
            let rerun = re_agent::find_executable("rerun")?;
            let current_exe = match &current_exe {
                Ok(exe) => exe.display().to_string(),
                Err(err) => format!("unknown ({err})"),
            };
            re_log::warn_once!(
                "The agent's MCP server runs a `rerun` from PATH instead of this executable, so it may be a different version. \
                 Current executable: {current_exe}\nMCP server executable: {}",
                rerun.display()
            );
            rerun
        }
    };

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
        let mut harness = Harness::default();

        // The checkout, not the directory the test happens to run in: `cargo` starts a test in
        // its own crate directory, well inside the checkout.
        let workspace = harness
            .user_like_cwd()
            .expect("a working directory to hide");
        assert!(
            std::env::current_dir()
                .expect("cwd")
                .starts_with(&workspace),
            "{workspace:?} must contain the directory the viewer was started from"
        );
        assert!(
            workspace
                .join("crates/panels/re_agent_panel/Cargo.toml")
                .is_file()
        );

        let scratch = harness
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
        let again = harness
            .user_like_cwd()
            .expect("a working directory to hide");
        assert_eq!(again, workspace);
        assert_eq!(
            harness
                .scratch_cwd
                .as_ref()
                .expect("a scratch directory")
                .path(),
            scratch
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
