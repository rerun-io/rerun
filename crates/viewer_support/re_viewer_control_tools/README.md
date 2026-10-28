# re_viewer_control_tools

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

The Rerun Viewer's control API (`viewer_control.proto`) as tools an LLM agent can call: one tool per operation, with descriptions and JSON schemas taken from the `.proto` comments.
Independent of how the tools reach the agent; `re_viewer_mcp` serves them over MCP.
