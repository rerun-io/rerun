//! Shows how to configure micro-batching directly from code.
//!
//! Check out <https://rerun.io/docs/reference/sdk/micro-batching> for more information.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Equivalent to configuring the following environment:
    // * RERUN_FLUSH_NUM_BYTES=<+inf>
    // * RERUN_FLUSH_NUM_ROWS=10
    let mut config = rerun::log::ChunkBatcherConfig::from_env().unwrap_or_default();
    config.flush_num_bytes = u64::MAX;
    config.flush_num_rows = 10;

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_micro_batching")
        .batcher_config(config)
        .spawn()?;

    // These 10 log calls are guaranteed be batched together, and end up in the same chunk.
    for i in 0..9 {
        rec.log("logs", &rerun::TextLog::new(format!("log #{i}")))?;
    }

    Ok(())
}
