use std::io::{Read, Seek};

use mcap::Summary;
use mcap::sans_io::{SummaryReadEvent, SummaryReader};
use re_chunk::TimePoint;
use re_log_types::{TimeCell, TimeType};
use saturating_cast::SaturatingCast as _;

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

/// Extracts log and publish time from an MCAP message as a `TimePoint`.
///
/// The `time_type` parameter controls whether the timelines are created as
/// [`TimeType::TimestampNs`] or [`TimeType::DurationNs`].
pub fn log_and_publish_timepoint_from_msg(
    msg: &mcap::Message<'_>,
    time_type: TimeType,
) -> TimePoint {
    let log_time_cell = crate::util::TimestampCell::from_nanos_default(msg.log_time, time_type);
    let publish_time_cell =
        crate::util::TimestampCell::from_nanos_default(msg.publish_time, time_type);
    re_chunk::TimePoint::from([
        ("message_log_time", log_time_cell.into_time_cell()),
        ("message_publish_time", publish_time_cell.into_time_cell()),
    ])
}

/// A timestamp or duration on a specific timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimestampCell {
    pub timeline: String,
    pub time: TimeCell,
}

impl TimestampCell {
    /// Create a Unix-epoch timestamp cell with a custom timeline name.
    ///
    /// Always interprets the value as a timestamp, regardless of magnitude.
    /// Use [`Self::from_nanos_with_type`] for configurable [`TimeType`].
    pub fn from_nanos(timestamp_ns: u64, timeline: impl Into<String>) -> Self {
        let ns = timestamp_ns.saturating_cast::<i64>();
        Self {
            timeline: timeline.into(),
            time: TimeCell::from_timestamp_nanos_since_epoch(ns),
        }
    }

    /// Create a time cell with a configurable [`TimeType`] and custom timeline name.
    pub fn from_nanos_with_type(
        nanos: u64,
        timeline: impl Into<String>,
        time_type: TimeType,
    ) -> Self {
        let ns = nanos.saturating_cast::<i64>();
        let time = match time_type {
            TimeType::TimestampNs => TimeCell::from_timestamp_nanos_since_epoch(ns),
            TimeType::DurationNs => TimeCell::from_duration_nanos(ns),
            TimeType::Sequence => TimeCell::from_sequence(ns),
        };
        Self {
            timeline: timeline.into(),
            time,
        }
    }

    /// Create a time cell on the `"timestamp"` timeline with the given [`TimeType`].
    pub fn from_nanos_default(timestamp_ns: u64, time_type: TimeType) -> Self {
        Self::from_nanos_with_type(timestamp_ns, "timestamp", time_type)
    }

    /// Create a time cell on the `"ros2_timestamp"` timeline with the given [`TimeType`].
    pub fn from_nanos_ros2(timestamp_ns: u64, time_type: TimeType) -> Self {
        Self::from_nanos_with_type(timestamp_ns, "ros2_timestamp", time_type)
    }

    /// The timeline name for this time cell.
    pub fn timeline_name(&self) -> &str {
        &self.timeline
    }

    /// Extract the contained [`TimeCell`].
    pub fn into_time_cell(self) -> TimeCell {
        self.time
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::cast_possible_wrap)] // ok in tests

    use re_log_types::TimeType;

    use super::*;

    #[test]
    fn test_from_nanos() {
        let ts: u64 = 1_672_531_200_000_000_000; // 2023-01-01
        let cell = TimestampCell::from_nanos_default(ts, TimeType::TimestampNs);
        assert_eq!(cell.timeline_name(), "timestamp");
        assert!(matches!(cell.time.typ, TimeType::TimestampNs));
        assert_eq!(
            cell.time,
            TimeCell::from_timestamp_nanos_since_epoch(ts as i64)
        );

        let cell = TimestampCell::from_nanos_default(ts, TimeType::DurationNs);
        assert_eq!(cell.timeline_name(), "timestamp");
        assert!(matches!(cell.time.typ, TimeType::DurationNs));
        assert_eq!(cell.time, TimeCell::from_duration_nanos(ts as i64));
    }

    #[test]
    fn test_from_nanos_ros2() {
        let ts: u64 = 1_672_531_200_000_000_000;
        let cell = TimestampCell::from_nanos_ros2(ts, TimeType::TimestampNs);
        assert_eq!(cell.timeline_name(), "ros2_timestamp");
        assert!(matches!(cell.time.typ, TimeType::TimestampNs));
    }

    #[test]
    fn test_from_nanos_custom_timeline() {
        let cell = TimestampCell::from_nanos(42, "my_timeline");
        assert_eq!(cell.timeline_name(), "my_timeline");
        assert_eq!(
            cell.into_time_cell(),
            TimeCell::from_timestamp_nanos_since_epoch(42)
        );
    }
}
