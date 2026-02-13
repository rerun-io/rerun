"""Log simple MCAP recording statistics."""

import rerun as rr

rr.init("rerun_example_mcap_statistics", spawn=True)

rr.log(
    "mcap/statistics/recording_overview",
    rr.McapStatistics(
        message_count=12500,
        schema_count=3,
        channel_count=5,
        attachment_count=2,
        metadata_count=8,
        chunk_count=25,
        message_start_time=1743465600000000000,  # 2024-04-01 00:00:00 UTC in nanoseconds
        message_end_time=1743466200000000000,  # 2024-04-01 00:10:00 UTC in nanoseconds (10 minute recording)
    ),
)
