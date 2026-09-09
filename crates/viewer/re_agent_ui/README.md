# re_agent_ui

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

A chat panel for talking to a coding agent from inside an egui app.
The agent (Claude Code, Codex, Gemini CLI, …) runs as a subprocess and speaks the
[Agent Client Protocol](https://agentclientprotocol.com/).
Any MCP servers configured in the panel are handed to the agent when the session starts,
so the agent can drive the Rerun Viewer through `rerun viewer-mcp`.

## Embedding

Hosts create an `AgentPanel` and call `ui()` every frame.
`AgentPanel::set_context` takes a `SessionContext` that every new session receives:
a markdown preamble sent together with the first prompt ("You are running inside…"),
and additional directories the agent may read.
Claude Code picks up skills from `.claude/skills/` inside those directories.

## Try it

```sh
cargo run -p re_agent_ui --example agent_app
```

Pick an installed agent, optionally point the MCP server entry at a running viewer, and start chatting.

## Iterating without a window

`--headless` renders the same UI through `egui_kittest` instead of opening a window.
Combine it with `EGUI_INSPECTION=1` and drive it with the `egui-mcp` MCP server (see `AGENTS.md` in `rerun/`):
click "Start", type prompts, answer permission prompts, and take screenshots.

```sh
EGUI_INSPECTION=1 cargo run -p re_agent_ui --example agent_app -- --headless --cwd /tmp
```

The snapshot tests in `tests/` cover the main screens with a scripted session; run them with `cargo test -p re_agent_ui --all-features`.
