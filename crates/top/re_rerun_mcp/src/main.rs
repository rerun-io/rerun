//! `rerun-mcp` — the standalone binary for the [`re_rerun_mcp`] MCP server.
//!
//! Mostly useful for rerun developers. Usually it's recommended to use `rerun mcp` instead.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    re_log::setup_logging();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(re_rerun_mcp::serve(re_rerun_mcp::Config::default()))?;
    Ok(())
}
