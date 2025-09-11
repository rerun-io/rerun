use std::io::{Read, Seek};

use mcap::{
    Summary,
    records::ChunkIndex,
    sans_io::{SummaryReadEvent, SummaryReader},
};
use re_chunk::external::nohash_hasher::IntMap;
use re_log_types::TimeCell;

use crate::parsers::ChannelId;

/// Read out the summary of an MCAP file.
pub fn read_summary<R: Read + Seek>(mut reader: R) -> anyhow::Result<Option<Summary>> {
    let mut summary_reader = SummaryReader::new();
    while let Some(event) = summary_reader.next_event() {
        match event? {
            SummaryReadEvent::SeekRequest(pos) => {
                summary_reader.notify_seeked(reader.seek(pos)?);
            }
            SummaryReadEvent::ReadRequest(need) => {
                let read = reader.read(summary_reader.insert(need))?;
                summary_reader.notify_read(read);
            }
        }
    }

    Ok(summary_reader.finish())
}

/// Counts the number of messages per channel within a specific chunk.
///
/// This function reads the message indexes for the given chunk and returns
/// a mapping of channel IDs to their respective message counts.
#[inline]
pub fn get_chunk_message_count(
    chunk_index: &ChunkIndex,
    summary: &Summary,
    mcap: &[u8],
) -> Result<IntMap<ChannelId, usize>, ::mcap::McapError> {
    Ok(summary
        .read_message_indexes(mcap, chunk_index)?
        .iter()
        .map(|(channel, msg_offsets)| (channel.id.into(), msg_offsets.len()))
        .collect())
}

/// Converts a raw timestamp (in nanoseconds) to a [`TimeCell`], making a best effort guess
/// about the epoch.
pub fn guess_epoch(timestamp_ns: u64) -> TimeCell {
    // Define reasonable bounds for Unix timestamps
    const YEAR_1990_NS: u64 = 631_148_400_000_000_000;
    const YEAR_2100_NS: u64 = 4_102_444_800_000_000_000;

    // If timestamp is within reasonable Unix range and close to current time
    if timestamp_ns >= YEAR_1990_NS && timestamp_ns <= YEAR_2100_NS {
        TimeCell::from_timestamp_nanos_since_epoch(timestamp_ns as i64)
    } else {
        // For custom epochs, use duration to represent relative time
        TimeCell::from_duration_nanos(timestamp_ns as i64)
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::TimeType;

    use super::*;

    #[test]
    fn test_guess_epoch() {
        // Test Unix timestamp within reasonable range (year 2023)
        let unix_timestamp_2023 = 1_672_531_200_000_000_000; // Jan 1, 2023
        let result = guess_epoch(unix_timestamp_2023);
        assert!(matches!(result.typ, TimeType::TimestampNs));

        // Test timestamp before 1990 (should use duration)
        let early_timestamp = 100_000_000; // Very small timestamp
        let result = guess_epoch(early_timestamp);
        assert!(matches!(result.typ, TimeType::DurationNs));

        // Test timestamp after 2100 (should use duration)
        let future_timestamp = 5_000_000_000_000_000_000; // Far future
        let result = guess_epoch(future_timestamp);
        assert!(matches!(result.typ, TimeType::DurationNs));

        // Test boundary cases
        let year_1990 = 631_148_400_000_000_000;
        let result = guess_epoch(year_1990);
        assert!(matches!(result.typ, TimeType::TimestampNs));

        let year_2100 = 4_102_444_800_000_000_000;
        let result = guess_epoch(year_2100);
        assert!(matches!(result.typ, TimeType::TimestampNs));

        // Test just outside boundaries
        let before_1990 = year_1990 - 1;
        let result = guess_epoch(before_1990);
        assert!(matches!(result.typ, TimeType::DurationNs));

        let after_2100 = year_2100 + 1;
        let result = guess_epoch(after_2100);
        assert!(matches!(result.typ, TimeType::DurationNs));
    }
}
