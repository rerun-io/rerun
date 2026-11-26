# Viewer agent context

This directory contains the Rerun context embedded in `re_agent_panel` and made available to the viewer agent.

- `skills` links to the Rerun agent skills.

`skills` is a symlink, and `cargo package` drops symlinked directories without warning.
A crate published from here therefore carries no skills, and `build.rs` prints a warning saying so.
The panel then runs without Rerun skills, which is a degraded but working panel — see the `agent_skills` feature of `re_agent_panel`.
