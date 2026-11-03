# re_agent_panel

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_agent_panel.svg)](https://crates.io/crates/re_agent_panel)
[![Documentation](https://docs.rs/re_agent_panel/badge.svg)](https://docs.rs/re_agent_panel)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

The agent panel of the Rerun Viewer: a chat with a coding agent that runs next to the viewer and drives it.

It configures the generic chat UI of `re_agent_ui` for the viewer.

## What goes here, and what does not

This crate is everything that ties the agent to the Rerun Viewer specifically.
UI that any egui app could use for talking to an agent goes in [`re_agent_ui`](../../ui_tools/re_agent_ui) instead, and the protocol and session state in `re_agent`.
