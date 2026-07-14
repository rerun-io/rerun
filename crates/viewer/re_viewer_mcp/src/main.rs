//! `re-viewer-mcp` — the standalone binary for the [`re_viewer_mcp`] MCP server.
//!
//! Mostly useful for rerun developers. Usually it's recommended to use `rerun viewer-mcp` instead.

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(re_viewer_mcp::serve())
}
