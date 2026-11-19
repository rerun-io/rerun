---
title: Agents
order: 2
description: Use a coding agent to log data, explore recordings, and customize the Rerun Viewer
keywords: agent, AI, LLM, Claude, Codex, MCP, skills, agent panel
---

You can work with a recording by chatting in the Rerun Viewer. If you already use a coding agent, you can give it tools to inspect the Viewer and skills to help it write Rerun code.

## MCP server

To work from your terminal or editor, connect your agent to the [Rerun MCP server](viewer/extend/mcp.md). It lets the agent inspect recordings, change what you see in the Viewer, and query datasets in a catalog.

## Skills

Rerun skills teach coding agents tasks such as converting MCAP files, designing blueprints, and querying datasets from Python. With Node.js installed, run the following command and choose the skills you want.

```sh
npx skills add rerun-io/rerun
```

Browse the [full list of Rerun skills](https://github.com/rerun-io/rerun/tree/main/skills).

## Agent panel

Click the agent button in the top right of the Viewer to open the experimental chat. Select an entity and ask “What data was logged for this?” or “Show this camera next to the 3D view”.

<picture>
  <source media="(max-width: 480px)" srcset="https://static.rerun.io/agent_panel_anomaly_scan/9dacd626a02e4ac29ec6fff6e1ce208d98465886/480w.png">
  <source media="(max-width: 768px)" srcset="https://static.rerun.io/agent_panel_anomaly_scan/9dacd626a02e4ac29ec6fff6e1ce208d98465886/768w.png">
  <source media="(max-width: 1024px)" srcset="https://static.rerun.io/agent_panel_anomaly_scan/9dacd626a02e4ac29ec6fff6e1ce208d98465886/1024w.png">
  <source media="(max-width: 1200px)" srcset="https://static.rerun.io/agent_panel_anomaly_scan/9dacd626a02e4ac29ec6fff6e1ce208d98465886/1200w.png">
  <img src="https://static.rerun.io/agent_panel_anomaly_scan/9dacd626a02e4ac29ec6fff6e1ce208d98465886/full.png" alt="The agent panel lists tracking anomalies beside camera views and sensor plots from the recording.">
</picture>

[Watch the agent investigate the recordings](https://static.rerun.io/f28ec42d7e0f6423ee585b343fcf9b0d9321567a_agent_panel_find_anomalies.mp4).

The native Viewer can run an installed coding agent. It supports Claude Code, Codex, Gemini CLI, GitHub Copilot CLI, OpenCode, Goose, and Cursor CLI. To use Claude Code or Codex in the panel, install Node.js as well.

The panel connects your agent to Rerun’s tools and skills. The agent can edit files and run code. The panel defaults to auto mode for Claude Code and agent mode for Codex. You can change the mode in the panel.

You can also connect directly to Anthropic or OpenAI. Set `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` before starting the native Viewer, or enter your key in the web Viewer.

Your model provider receives your prompts and tool results, which can include screenshots and recording data.
