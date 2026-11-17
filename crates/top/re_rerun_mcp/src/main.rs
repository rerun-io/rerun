//! `rerun-mcp` — the standalone binary for the [`re_rerun_mcp`] MCP server.
//!
//! Mostly useful for rerun developers. Usually it's recommended to use `rerun mcp` instead.

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();
    let rt = tokio::runtime::Builder::new_multi_thread() // NOLINT: the standalone process owns this runtime
        .enable_all()
        .build()?;
    rt.block_on(re_rerun_mcp::serve(re_rerun_mcp::Config::default()))
}
