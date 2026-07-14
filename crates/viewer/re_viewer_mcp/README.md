# re_viewer_mcp

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

MCP server for the Rerun Viewer. See the [docs](https://rerun.io/docs/reference/viewer/mcp) for more info.

## Development

There is a `.mcp.json` that Claude should pick up in the Rerun repository root.

Use `cargo build -p re_viewer_mcp` to build the updated mcp server, and then within claude use `/mcp` and select `rerun` and
then reconnect, and it'll use the updated mcp (or reboot the cli).
