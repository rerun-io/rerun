//! Log simple MCAP recording statistics.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_mcap_statistics").spawn()?;

    rec.log(
        "mcap/statistics/recording_overview",
        &rerun::McapStatistics::update_fields()
            .with_message_count(12500u64)
            .with_schema_count(3u64)
            .with_channel_count(5u64)
            .with_attachment_count(2u64)
            .with_metadata_count(8u64)
            .with_chunk_count(25u64)
            .with_message_start_time(1743465600000000000i64) // 2024-04-01 00:00:00 UTC in nanoseconds
            .with_message_end_time(1743466200000000000i64), // 2024-04-01 00:10:00 UTC in nanoseconds (10 minute recording)
    )?;

    Ok(())
}
