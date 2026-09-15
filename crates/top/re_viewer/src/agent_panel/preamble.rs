//! Instructions sent to the viewer agent with its first prompt.

use std::fmt::Write as _;
use std::path::Path;

use super::{DOCS_SUBDIR, SKILLS_SUBDIR};

/// Build the instructions for the viewer agent.
pub fn text(viewer_endpoint: Option<&str>, agent_dir: Option<&Path>) -> String {
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

    if let Some(agent_dir) = agent_dir {
        writeln!(
            text,
            "- Rerun skills (data model, blueprints, MCAP, LeRobot, …) are in `{}`. \
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
        "- When a recording has joint angles but no robot model, suggest adding a URDF: \
         fetch one for that robot, solve forward kinematics from the joint states, \
         and layer the transforms onto the recording. The `rerun-urdf` skill has the sources and the pipeline.\n",
    );

    text.push_str("- Teach the user how to do things they could have easily done themselves.\n");
    text.push_str(
        "- Point users to scripts you wrote for them, to teach them how to use the Rerun SDK.\n",
    );
    text.push_str("- Keep answers short: the panel is narrow.\n");
    text
}
