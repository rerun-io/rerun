# re_agent

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Runs a coding agent (Claude Code, Codex, Gemini CLI, …) as a subprocess and talks to it over the
[Agent Client Protocol](https://agentclientprotocol.com/): sessions, prompts, permissions, and a
transcript model. No UI; see `re_agent_ui` for the egui chat panel built on top.

The agent stays the user's agent: it starts with its own system prompt and loads its usual
configuration from the working directory and the user's home (`CLAUDE.md`/`AGENTS.md`, skills,
slash commands, MCP servers). Whatever the host adds through `SessionContext` and
`McpServerConfig` comes on top of that.
