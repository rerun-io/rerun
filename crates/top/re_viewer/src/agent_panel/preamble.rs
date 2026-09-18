//! Instructions sent to the viewer agent with its first prompt.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::{DOCS_SUBDIR, SKILLS_SUBDIR};

/// Everything about the host that changes what the agent is told.
#[derive(Default)]
pub struct Preamble<'a> {
    /// The gRPC address of this viewer, so the agent can reconnect if its MCP server is not
    /// already attached. Unset when the viewer serves no endpoint to connect to.
    pub viewer_endpoint: Option<&'a str>,

    /// Where the skills and docs were unpacked for the agent to read.
    /// Unset when there was nowhere to unpack them, in which case they go unmentioned.
    pub agent_dir: Option<&'a Path>,

    /// Directories to ask the agent to stay out of, named in the instructions.
    ///
    /// The same list the session is given as [`re_agent_ui::SessionContext::off_limits_directories`],
    /// so that what is asked for and what is reported as a breach cannot drift apart.
    ///
    /// Holds the Rerun checkout in our own development builds, where the viewer is usually started
    /// from it: without the request the agent answers from the source code, which no user of a
    /// released build can do, and we would be testing a version of the panel that we do not ship.
    pub off_limits_directories: &'a [PathBuf],
}

impl Preamble<'_> {
    /// Build the instructions for the viewer agent.
    pub fn text(&self) -> String {
        let Self {
            viewer_endpoint,
            agent_dir,
            off_limits_directories,
        } = self;

        let mut text = String::from(
            "You are a helpful Rerun agent, running in a chat panel inside the Rerun Viewer \
             that the user is looking at right now. You help the user understand their data, \
             use the viewer, set up blueprints, and debug their Rerun logging code.\n\n",
        );

        // The MCP server only dials the viewer on startup when it was given an endpoint, so without
        // one the agent must be told to connect rather than told not to.
        if let Some(endpoint) = viewer_endpoint {
            write!(
                text,
                "- The `rerun` MCP server is already connected to this very viewer: do not call `rerun_connect`. \
                 Only if a tool reports that it is not connected, call `rerun_connect` with the endpoint `{endpoint}`."
            )
            .ok();
        } else {
            text.push_str(
                "- The `rerun` MCP server drives this very viewer, but is not connected yet: \
                 call `rerun_connect` before its other tools.",
            );
        }
        text.push_str(
            " Use its tools to see what the user sees and to drive the viewer, instead of guessing. \
             Prefer the high-level `rerun_*` tools (`rerun_get_viewer_state`, `rerun_set_time_cursor`, …); \
             drop to the low-level widget tools (`query_tree`, `click`, `screenshot`, …) only for what they do not cover.\n",
        );

        if let Some(agent_dir) = *agent_dir {
            writeln!(
                text,
                "- Rerun skills (data model, blueprints, MCAP, LeRobot, …) are in `{}`, \
                 indexed by the `README.md` there, which says in one line what each one covers. \
                 Read the relevant `SKILL.md` before answering questions on those topics.",
                agent_dir.join(SKILLS_SUBDIR).display()
            )
            .ok();
            writeln!(
                text,
                "- The Rerun documentation (the source of rerun.io/docs) and its code snippets are in `{}`. \
                 The `rerun-docs` skill explains the layout; grep there before answering how Rerun works.",
                agent_dir.join(DOCS_SUBDIR).display()
            )
            .ok();
        }

        text.push_str(
            "- Assume no Python environment is set up: the skills and the docs snippets are written in \
             Python, but a bare `python` here usually has neither `rerun` nor the packages they import. \
             Where `uv` is installed, `uv run --with rerun-sdk --with <other packages> python …` needs no \
             setup at all — note that the SDK installs as `rerun-sdk` and imports as `rerun`. \
             Check what you have before you write a script that assumes it.\n",
        );

        text.push_str(
            "- Run Python from a directory you control, and set `PYTHONSAFEPATH=1` where the \
             interpreter is 3.11 or newer. Without it the interpreter puts the script's directory \
             first on `sys.path`, so a stray `inspect.py` or `types.py` left there by someone else \
             shadows the standard library and is executed on the next import; older interpreters \
             ignore the variable, so there the directory you run from is the whole defense. \
             When you need a directory to work in, make a fresh one rather than reusing a path you \
             picked by hand — the invented name is the one another session picks too. `mktemp -d` \
             where you have it, otherwise `python -c \"import tempfile; print(tempfile.mkdtemp())\"`, \
             which works anywhere Python does.\n",
        );

        text.push_str(
            "- Assume nothing about the machine you are on: Linux, macOS or Windows; a container or a \
             sandbox; `bash`, `zsh` or something else; any set of installed tools; possibly no network. \
             Check before you depend on something, keep to portable commands, and read the error you \
             got rather than assuming the command was wrong. Quoting is part of this — a `zsh` with \
             `EQUALS` expansion on reads a bare `echo ====` as a command name and fails with \
             `zsh:1: === not found`, which stops the rest of an `&&` chain.\n",
        );

        text.push_str(
            "- Rerun is open source, and its examples are worth reading and worth linking: \
             <https://rerun.io/examples> shows each one, and its code is under `examples/` in \
             <https://github.com/rerun-io/rerun>. Larger applications — SLAM, gaussian splatting, \
             segmentation, depth — are one runnable package each in the Pixi workspace at \
             <https://github.com/rerun-io/examples-monorepo>. Point the user at the example that \
             matches what they are building rather than describing one in prose.\n",
        );

        text.push_str(
            "- When a recording has joint angles but no robot model, suggest adding a URDF: \
             fetch one for that robot, solve forward kinematics from the joint states, \
             and layer the transforms onto the recording. The `rerun-urdf` skill has the sources and the pipeline.\n",
        );

        text.push_str(
            "- Say what you are about to do before any command that may run for minutes — a download, \
             an import, a build. The user is watching a panel that shows nothing while a tool runs, so \
             silence reads as a hang. One sentence naming the thing and its rough size is enough.\n",
        );

        text.push_str(
            "- Teach the user how to do things they could have easily done themselves.\n",
        );
        text.push_str(
            "- Point users to scripts you wrote for them, to teach them how to use the Rerun SDK.\n",
        );
        text.push_str("- Keep answers short: the panel is narrow.\n");

        if !off_limits_directories.is_empty() {
            let directories = off_limits_directories
                .iter()
                .map(|directory| format!("`{}`", directory.display()))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(
                text,
                "- Do NOT read the source code of Rerun, even if you find it on disk, unless you \
                 are explicitly asked to. It is in {directories}: stay out of there with every \
                 tool, a shell command as much as a file read. Pretend that you only have the \
                 access that a user of a released build would have."
            )
            .ok();
        }

        text
    }
}
