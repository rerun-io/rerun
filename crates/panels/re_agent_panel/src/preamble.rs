//! Instructions sent to the viewer agent with its first prompt.
//!
//! Only things specific to the agent panel should go into this file.
//! Anything general to rerun-mcp goes in the `re_rerun_mcp` preamble instead.

#[cfg(agent_harness_backend)]
use std::fmt::Write as _;
#[cfg(agent_harness_backend)]
use std::path::{Path, PathBuf};

#[cfg(agent_harness_backend)]
use super::SKILLS_SUBDIR;

/// The start of the instructions, the same for every backend.
fn intro() -> String {
    let p = re_viewer_control_tools::TOOL_NAME_PREFIX;
    format!(
        "You are a helpful Rerun agent, running in a chat panel inside the Rerun Viewer \
         that the user is looking at right now. You help the user understand their data, \
         use the viewer, set up blueprints, and debug their Rerun logging code.\n\n\
         - Before calling a tool, inspect its input schema; don't infer argument names or types \
         from the tool name. If discovery is needed, fetch the schema first.\n\
         - When working with blueprints, use these tools to understand the context and edit \
         the blueprint: `{p}get_state`, `{p}get_recording_schema`, `{p}get_blueprint`, and \
         `{p}set_blueprint`.\n"
    )
}

/// The end of the instructions, the same for every backend.
const OUTRO: &str = "- Before answering how Rerun works, read the documentation for the version of this \
     viewer, which `viewer_get_state` reports: the docs source at \
     `https://raw.githubusercontent.com/rerun-io/rerun/<version>/docs/` and the Python API reference \
     at `https://ref.rerun.io/docs/python/<version>/`. Use `main` for a development build. \
     <https://rerun.io/docs> shows only the latest release, which can be newer than this viewer.\n\
     - Rerun is open source, and its examples are worth reading and worth linking: \
     their code is under `examples/` in <https://github.com/rerun-io/rerun>, and \
     <https://rerun.io/examples> walks through collecting, preparing, and training on robot \
     data. Larger applications — SLAM, gaussian splatting, \
     segmentation, depth — are one runnable package each in the Pixi workspace at \
     <https://github.com/rerun-io/examples-monorepo>. Point the user at the example that \
     matches what they are building rather than describing one in prose.\n\
     - Teach the user how to do things they could have easily done themselves.\n\
     - Keep answers short: the panel is narrow. Omit internal tool commentary unless the user asks for it.\n";

/// Everything about the host that changes what the agent is told.
#[cfg(agent_harness_backend)]
#[derive(Default)]
pub struct Preamble<'a> {
    /// The gRPC address of this viewer, so the agent can reconnect if its MCP server is not
    /// already attached. Unset when the viewer serves no endpoint to connect to.
    pub viewer_endpoint: Option<&'a str>,

    /// Where the skills were unpacked for the agent to read.
    /// Unset when there was nowhere to unpack them, in which case they go unmentioned.
    pub agent_dir: Option<&'a Path>,

    /// Directories to ask the agent to stay out of, named in the instructions.
    ///
    /// The same list the session is given as [`re_agent::SessionContext::off_limits_directories`],
    /// so that what is asked for and what is reported as a breach cannot drift apart.
    ///
    /// Holds the Rerun checkout in our own development builds, where the viewer is usually started
    /// from it: without the request the agent answers from the source code, which no user of a
    /// released build can do, and we would be testing a version of the panel that we do not ship.
    pub off_limits_directories: &'a [PathBuf],
}

#[cfg(agent_harness_backend)]
impl Preamble<'_> {
    /// Build the instructions for the viewer agent.
    pub fn text(&self) -> String {
        let Self {
            viewer_endpoint,
            agent_dir,
            off_limits_directories,
        } = self;

        let mut text = intro();

        // The MCP server only dials the viewer on startup when it was given an endpoint, so without
        // one the agent must be told to connect rather than told not to.
        if let Some(endpoint) = viewer_endpoint {
            write!(
                text,
                "- The `rerun` MCP server is already connected to this very viewer: do not call `viewer_connect`. \
                 Only if a tool reports that it is not connected, call `viewer_connect` with the endpoint `{endpoint}`."
            )
            .ok();
        } else {
            text.push_str(
                "- The `rerun` MCP server drives this very viewer, but is not connected yet: \
                 call `viewer_connect` before its other tools.",
            );
        }
        text.push_str(
            " Use its tools to see what the user sees and to drive the viewer, instead of guessing. \
             Prefer the high-level `viewer_*` tools (`viewer_get_state`, `viewer_set_time_cursor`, …), \
             and `viewer_run_command` for anything the command palette can do (its `id` lists every command); \
             drop to the low-level `ui_*` widget tools (`ui_widget_tree`, `ui_click`, `ui_screenshot`, …) only for what they do not cover. \
             The panel hands the `rerun` server to this session directly, so it never shows up in your own MCP \
             configuration, with tools as `viewer_*`.\n",
        );
        text.push_str(
            "- If your harness defers `viewer_*` tool schemas (e.g. `ToolSearch` in Claude Code), \
             load them before the first call.\n",
        );

        if let Some(agent_dir) = *agent_dir {
            writeln!(
                text,
                "- Rerun skills (data model, blueprints, catalog/dataset queries, MCAP, LeRobot, …) are in `{}`. \
                 Always check its `README.md` first to see which skills are available, then read the relevant `SKILL.md` \
                 before starting the task.",
                agent_dir.join(SKILLS_SUBDIR).display()
            )
            .ok();
            text.push_str(
                "- To change how a table looks (previews, cards, columns, filters), read the `rerun-blueprint` skill \
                 and call `viewer_set_blueprint`. Do not click through the UI.\n",
            );
        }

        text.push_str(
            "- Use elicitation whenever you need clarification, a choice, or additional input from \
             the user, for any task. Call your agent's built-in structured question tool and wait \
             for the answer instead of ending your response with a plain-text question. \
             Examples are `AskUserQuestion` for Claude and synchronous `request_user_input` for Codex; \
             these are agent-native tools, not Rerun MCP tools. Ask in chat only when the tool is \
             unavailable, rejects the request, or cannot collect the needed input (such as a \
             text-only answer when it requires choices). Do not use `request_user_input_async` \
             with this panel.\n",
        );

        text.push_str(
            "- Before running Python, ask once with a structured choice question: existing environment or \
             a temporary one, unless the user already specified it. Use elicitation through your agent's \
             structured question tool, e.g. synchronous `request_user_input` for Codex or `AskUserQuestion` \
             for Claude. For an existing environment, get an absolute \
             interpreter path or the project directory and run command. Resolve relative paths \
             against that project, not a scratch directory; do not guess environment paths. \
             Reuse the chosen environment, check required imports, and discuss missing dependencies \
             before changing it. For a temporary environment, use \
             `uv run --with 'rerun-sdk[catalog]' --with <other packages> python …` when available. \
             The SDK installs as `rerun-sdk` and imports as `rerun`. \
             Give users that same install line. \
             Pin the SDK to the viewer's minor version: read `viewer_version` from \
             `viewer_get_state`, and use `rerun-sdk[catalog]~=0.X.0`. Check `rerun.__version__` in your first run, \
             and do not trust an unpinned resolve or an inherited `VIRTUAL_ENV`.\n",
        );

        text.push_str(
            "- Run Python from a directory you control. Set `PYTHONSAFEPATH=1` for Python 3.11+ \
             to avoid local files shadowing the standard library. Use fresh temporary directories \
             for disposable work; save reusable scripts somewhere persistent.\n",
        );

        text.push_str(
            "- Check which tools are available and use commands appropriate to the host platform and shell.\n",
        );

        text.push_str(
            "- When a recording has joint angles but no robot model, suggest adding a robot model (URDF or MJCF): \
             fetch one for that robot, solve forward kinematics from the joint states, \
             and layer the transforms onto the recording. The `rerun-robot-model` skill has the sources and the pipeline.\n",
        );

        text.push_str(
            "- Say what you are about to do before any command that may run for minutes — a download, \
             an import, a build. The user is watching a panel that shows nothing while a tool runs, so \
             silence reads as a hang. One sentence naming the thing and its rough size is enough.\n",
        );

        text.push_str(
            "- Point users to scripts you wrote for them, to teach them how to use the Rerun SDK.\n",
        );
        text.push_str(OUTRO);

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

/// Instructions for an agent whose tools act on this viewer directly.
pub fn model_api_text() -> String {
    let mut text = intro();
    text.push_str(
        "- Your `viewer_*` tools act on this viewer directly and are ready to use. Start with \
         `viewer_get_state` to see the loaded recordings, timelines, and views. Use \
         `viewer_run_command` for anything the command palette can do: its `id` lists every command.\n\
         - Use `viewer_save_screenshot` to inspect appearance and framing. The result is an image. \
         Check state with `viewer_get_state` or `viewer_get_blueprint` first. A full-window screenshot is large.\n\
         - Your `catalog_*` tools read datasets and tables on a Rerun catalog server, by default \
         this viewer's own catalog (`catalog_url` in `viewer_get_state`), which holds the `.rrd` \
         files it opened. Recordings missing there are only in the viewer. Use `catalog_connect` \
         for another server",
    );
    if cfg!(not(target_arch = "wasm32")) {
        text.push_str(
            ", e.g. one from `catalog_list_servers`, and `catalog_login` when it needs a login",
        );
    }
    text.push_str(
        ". Read values with `catalog_query_dataset` rather than asking for code to run.\n",
    );
    text.push_str("- ");
    text.push_str(&re_catalog_tools::CatalogTool::write_guidance());
    text.push_str(
        "\n- Ask for clarification in chat when needed. Provide code for the user to run when a \
         task needs their local files or Python environment.\n",
    );
    if crate::skills_toolbox::SkillsToolbox::embedded().is_some() {
        text.push_str(
            "- Rerun skills are listed in the description of `skill_read`. \
             Read the relevant one with it before starting the task.\n",
        );
    }
    text.push_str(OUTRO);
    text
}

#[cfg(test)]
mod tests {
    #[test]
    fn model_api_instructions_name_available_tools() {
        let quoted = super::model_api_text();
        // `catalog_url` is a field of `viewer_get_state`, not a tool.
        let quoted_names = quoted
            .split('`')
            .skip(1)
            .step_by(2)
            .filter(|name| name.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
        for name in quoted_names {
            if let Some(operation) = name.strip_prefix(re_viewer_control_tools::TOOL_NAME_PREFIX) {
                assert!(
                    re_viewer_control_tools::is_operation(operation),
                    "unknown tool: {name}"
                );
            } else if name.starts_with("skill_") {
                assert_eq!(name, crate::skills_toolbox::SKILL_READ, "unknown tool");
            } else if name.starts_with("catalog_") && name != "catalog_url" {
                assert!(
                    re_catalog_tools::CatalogTool::from_name(name).is_some(),
                    "unknown tool: {name}"
                );
            }
        }
    }
}
