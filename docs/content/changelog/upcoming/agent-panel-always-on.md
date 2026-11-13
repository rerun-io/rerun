---
title: Agent panel
hidden: true
type: feature
---

### Agent panel

The Viewer has a new agent panel: chat with a coding agent that can inspect and control the Viewer through the [Viewer MCP tools](../viewer/extend/mcp.md).
Open it with the agent button in the top right corner, or press Tab in the command palette to ask the agent directly.
The panel is still experimental, and we would love your feedback.

The panel runs the agent in one of two ways.

#### Your own coding agent (native Viewer only)

The panel starts a coding agent that is installed on your computer, such as Claude Code, Codex, Gemini CLI, GitHub Copilot CLI, OpenCode, Goose, or Cursor CLI.
The agent drives the Viewer through the Rerun MCP server.
This is the most capable option:
- It is your agent, with your subscription, so you need no extra API key.
- It keeps your configuration, such as `CLAUDE.md`/`AGENTS.md`, skills, and your other MCP servers.
- It can also work with files and run code on your computer, for example to write a script that logs more data.

#### Model API

The panel can also run the agent itself and call the Anthropic or OpenAI API directly.
This works in both the native and the web Viewer, and needs no agent installed.
On native, set `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` in your environment; on the web, enter your key in the panel.
This agent can only use the Viewer tools, not your files or your own agent configuration.

Previous releases shipped an early version of the panel behind an experimental setting.

<video width="100%" autoplay loop muted controls>
    <source src="https://static.rerun.io/f28ec42d7e0f6423ee585b343fcf9b0d9321567a_agent_panel_find_anomalies.mp4" type="video/mp4" />
</video>
