# Viewer agent context

This directory contains the Rerun context embedded in `re_viewer` and made available to the viewer agent.

- `docs` links to the Rerun documentation and its code snippets.
- `skills` links to the Rerun agent skills.

Both entries are symlinks, and `cargo package` drops symlinked directories without warning.
A crate published from here therefore carries no agent context, and `build.rs` prints a warning
saying so.
The panel then runs without Rerun skills or documentation, which is a degraded but working
panel — see the `agent_context` feature of `re_viewer`.
