// Log simple MCAP recording statistics.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_mcap_statistics");
    rec.spawn().exit_on_failure();

    rec.log(
        "mcap/statistics/recording_overview",
        rerun::archetypes::McapStatistics::update_fields()
            .with_message_count(12500)
            .with_schema_count(3)
            .with_channel_count(5)
            .with_attachment_count(2)
            .with_metadata_count(8)
            .with_chunk_count(25)
            .with_message_start_time(1743465600000000000) // 2024-04-01 00:00:00 UTC in nanoseconds
            .with_message_end_time(
                1743466200000000000 // 2024-04-01 00:10:00 UTC in nanoseconds (10 minute recording)
            )

    );
}
