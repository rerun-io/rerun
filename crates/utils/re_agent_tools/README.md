# re_agent_tools

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

The interface between an LLM agent and the tools a host offers it: `AgentToolbox`.

A toolbox does not care how it reaches the agent.
The viewer's agent panel hands toolboxes to a model-API agent in-process, and `re_rerun_mcp` serves them to coding agents over MCP.
