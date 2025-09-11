use std::io::{Read, Seek};

use mcap::{
    Summary,
    sans_io::{SummaryReadEvent, SummaryReader},
};
use re_log_types::TimeCell;

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

/// Timestamp + epoch interpretation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimestampCell {
    /// Unix epoch (nanoseconds since 1970-01-01).
    Unix { time: TimeCell },

    /// User-understood epoch with a named timeline (nanoseconds since custom zero).
    Custom { timeline: String, time: TimeCell },
}

impl TimestampCell {
    // Unix range we consider "reasonable" for raw ns values.
    const YEAR_1990_NS: i64 = 631_148_400_000_000_000; // 1990-01-01
    const YEAR_2100_NS: i64 = 4_102_444_800_000_000_000; // 2100-01-01

    /// Make a best-effort guess on the epoch type based on the provided raw timestamp.
    pub fn guess_from_nanos(timestamp_ns: u64, timeline: impl Into<String>) -> Self {
        let ns = timestamp_ns as i64; // safe for our bounds

        if ns >= Self::YEAR_1990_NS && ns <= Self::YEAR_2100_NS {
            Self::Unix {
                time: TimeCell::from_timestamp_nanos_since_epoch(ns),
            }
        } else {
            Self::Custom {
                timeline: timeline.into(),
                time: TimeCell::from_duration_nanos(ns),
            }
        }
    }

    /// The timeline name for this timestamp.
    pub fn timeline_name(&self) -> &str {
        match self {
            Self::Unix { .. } => "timestamp",
            Self::Custom { timeline, .. } => timeline,
        }
    }

    /// Extract the contained [`TimeCell`].
    pub fn into_time_cell(self) -> TimeCell {
        match self {
            Self::Unix { time } | Self::Custom { time, .. } => time,
        }
    }
}

#[cfg(test)]
mod tests {
    use re_log_types::TimeType;

    use super::*;

    #[test]
    fn test_guess_from_nanos() {
        // within reasonable unix range for `TimestampCell::Unix`
        let unix_ts_2023: u64 = 1_672_531_200_000_000_000; // 2023-01-01
        let cell = TimestampCell::guess_from_nanos(unix_ts_2023, "test_timeline");
        let TimestampCell::Unix { time } = cell else {
            panic!("expected `TimestampCell::Unix` variant")
        };

        assert!(matches!(time.typ, TimeType::TimestampNs));
        assert_eq!(
            time,
            TimeCell::from_timestamp_nanos_since_epoch(unix_ts_2023 as i64)
        );
        assert_eq!(TimestampCell::Unix { time }.timeline_name(), "timestamp");

        // early date for `TimestampCell::Custom`
        let early: u64 = 100_000_000;
        let cell = TimestampCell::guess_from_nanos(early, "test_timeline");
        let TimestampCell::Custom { timeline, time } = cell else {
            panic!("expected `TimestampCell::Custom` variant")
        };
        assert_eq!(timeline, "test_timeline");
        assert!(matches!(time.typ, TimeType::DurationNs));
        assert_eq!(time, TimeCell::from_duration_nanos(early as i64));

        // after 2100 for `TimestampCell::Custom`
        let far_future: u64 = 5_000_000_000_000_000_000;
        let cell = TimestampCell::guess_from_nanos(far_future, "test_timeline");
        let TimestampCell::Custom { timeline, time } = cell else {
            panic!("expected `TimestampCell::Custom` variant")
        };
        assert_eq!(timeline, "test_timeline");
        assert!(matches!(time.typ, TimeType::DurationNs));
        assert_eq!(time, TimeCell::from_duration_nanos(far_future as i64));

        // exactly 1990-01-01 for `TimestampCell::Unix`
        let year_1990 = TimestampCell::YEAR_1990_NS as u64;
        let cell = TimestampCell::guess_from_nanos(year_1990, "test_timeline");
        let TimestampCell::Unix { time } = cell else {
            panic!("expected `TimestampCell::Unix` at lower boundary")
        };
        assert!(matches!(time.typ, TimeType::TimestampNs));
        assert_eq!(
            time,
            TimeCell::from_timestamp_nanos_since_epoch(year_1990 as i64)
        );

        // exactly 2100-01-01 for `TimestampCell::Unix`
        let year_2100 = TimestampCell::YEAR_2100_NS as u64;
        let cell = TimestampCell::guess_from_nanos(year_2100, "test_timeline");
        let TimestampCell::Unix { time } = cell else {
            panic!("expected `TimestampCell::Unix` at upper boundary")
        };
        assert!(matches!(time.typ, TimeType::TimestampNs));
        assert_eq!(
            time,
            TimeCell::from_timestamp_nanos_since_epoch(year_2100 as i64)
        );

        // just outside lower boundary for `TimestampCell::Custom`
        let before_1990 = (TimestampCell::YEAR_1990_NS - 1) as u64;
        let cell = TimestampCell::guess_from_nanos(before_1990, "test_timeline");
        let TimestampCell::Custom { timeline, time } = cell else {
            panic!("expected `TimestampCell::Custom` just before lower boundary")
        };
        assert_eq!(timeline, "test_timeline");
        assert!(matches!(time.typ, TimeType::DurationNs));
        assert_eq!(time, TimeCell::from_duration_nanos(before_1990 as i64));

        // just outside upper boundary for `TimestampCell::Custom`
        let after_2100 = (TimestampCell::YEAR_2100_NS + 1) as u64;
        let cell = TimestampCell::guess_from_nanos(after_2100, "test_timeline");
        let TimestampCell::Custom { timeline, time } = cell else {
            panic!("expected `TimestampCell::Custom` just after upper boundary")
        };
        assert_eq!(timeline, "test_timeline");
        assert!(matches!(time.typ, TimeType::DurationNs));
        assert_eq!(time, TimeCell::from_duration_nanos(after_2100 as i64));
    }

    #[test]
    fn test_timeline_name() {
        let unix = TimestampCell::Unix {
            time: TimeCell::from_timestamp_nanos_since_epoch(1_234_567_890),
        };
        assert_eq!(unix.timeline_name(), "timestamp");

        let custom = TimestampCell::Custom {
            timeline: "sensor/imu".to_owned(),
            time: TimeCell::from_duration_nanos(1_234_567_890),
        };
        assert_eq!(custom.timeline_name(), "sensor/imu");
    }

    #[test]
    fn test_into_time_cell() {
        let timestamp1 = TimeCell::from_timestamp_nanos_since_epoch(42);
        let unix = TimestampCell::Unix { time: timestamp1 };
        assert_eq!(unix.into_time_cell(), timestamp1);

        let timestamp2 = TimeCell::from_duration_nanos(1337);
        let custom = TimestampCell::Custom {
            timeline: "foo".into(),
            time: timestamp2,
        };
        assert_eq!(custom.into_time_cell(), timestamp2);
    }
}
