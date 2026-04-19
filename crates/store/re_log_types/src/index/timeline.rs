use crate::{AbsoluteTimeRange, TimeType, TimelineName, TimestampFormat};

// ----------------------------------------------------------------------------

/// A time frame/space, e.g. `log_time` or `frame_nr`, coupled with the type of time
/// it keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Timeline {
    /// Name of the timeline (e.g. `log_time`).
    name: TimelineName,

    /// Sequence or time?
    typ: TimeType,
}

impl Timeline {
    #[inline]
    pub fn new(name: impl Into<TimelineName>, typ: TimeType) -> Self {
        Self {
            name: name.into(),
            typ,
        }
    }

    /// For things like camera frames or iteration count.
    #[inline]
    pub fn new_sequence(name: impl Into<TimelineName>) -> Self {
        Self {
            name: name.into(),
            typ: TimeType::Sequence,
        }
    }

    /// For relative times (e.g. seconds since start).
    #[inline]
    pub fn new_duration(name: impl Into<TimelineName>) -> Self {
        Self {
            name: name.into(),
            typ: TimeType::DurationNs,
        }
    }

    /// For absolute timestamps.
    #[inline]
    pub fn new_timestamp(name: impl Into<TimelineName>) -> Self {
        Self {
            name: name.into(),
            typ: TimeType::TimestampNs,
        }
    }

    #[inline]
    pub fn name(&self) -> &TimelineName {
        &self.name
    }

    #[inline]
    pub fn typ(&self) -> TimeType {
        self.typ
    }

    /// The log time timeline to which all API functions will always log.
    ///
    /// This timeline is automatically maintained by the SDKs and captures the wall-clock time at
    /// which point the data was logged (according to the client's wall-clock).
    #[inline]
    pub fn log_time() -> Self {
        Self::new(TimelineName::log_time(), TimeType::TimestampNs)
    }

    /// The log tick timeline to which all API functions will always log.
    ///
    /// This timeline is automatically maintained by the SDKs and captures the logging tick at
    /// which point the data was logged.
    /// The logging tick is monotically incremented each time the client calls one of the logging
    /// methods on a `RecordingStream`.
    #[inline]
    pub fn log_tick() -> Self {
        Self::new(TimelineName::log_tick(), TimeType::Sequence)
    }

    /// Returns a formatted string of `time_range` on this `Timeline`.
    #[inline]
    pub fn format_time_range(
        &self,
        time_range: &AbsoluteTimeRange,
        timestamp_format: TimestampFormat,
    ) -> String {
        self.typ.format_range(*time_range, timestamp_format)
    }

    /// Returns a formatted string of `time_range` on this `Timeline`.
    #[inline]
    pub fn format_time_range_utc(&self, time_range: &AbsoluteTimeRange) -> String {
        self.format_time_range(time_range, TimestampFormat::utc())
    }

    /// Returns the appropriate arrow datatype to represent this timeline.
    #[inline]
    pub fn datatype(&self) -> arrow::datatypes::DataType {
        self.typ.datatype()
    }

    /// Whether this is a built-in timeline (`log_time` or `log_tick`) as opposed to a
    /// user-defined one.
    #[inline]
    pub fn is_builtin(&self) -> bool {
        *self == Self::log_time() || *self == Self::log_tick()
    }

    /// Pick the most likely "default" timeline from a set of candidates.
    ///
    /// Priority (highest first):
    /// 1. `message_log_time` (present in MCAP imports, common in robotics)
    /// 2. Other user-defined timelines
    /// 3. `log_time`
    /// 4. `log_tick`
    ///
    /// Among timelines of the same priority, the one with the higher `score` wins
    /// (e.g. row count).
    /// Falls back to `log_time` if the iterator is empty.
    pub fn pick_best_timeline<'a>(
        timelines: impl IntoIterator<Item = &'a Self>,
        score: impl Fn(&Self) -> u64,
    ) -> Self {
        fn priority(timeline: &Timeline) -> u8 {
            if timeline.name().as_str() == "message_log_time" {
                3
            } else if *timeline == Timeline::log_tick() {
                0
            } else if *timeline == Timeline::log_time() {
                1
            } else {
                2 // user-defined
            }
        }

        timelines
            .into_iter()
            .max_by(|a, b| {
                priority(a)
                    .cmp(&priority(b))
                    .then_with(|| score(a).cmp(&score(b)))
            })
            .copied()
            .unwrap_or_else(Self::log_time)
    }
}

impl nohash_hasher::IsEnabled for Timeline {}

impl re_byte_size::SizeBytes for Timeline {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

// required for [`nohash_hasher`].
impl std::hash::Hash for Timeline {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.name.hash() ^ self.typ.hash());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pick_best_timeline() {
        let log_time = Timeline::log_time();
        let log_tick = Timeline::log_tick();
        let custom_timeline0 = Timeline::new("my_timeline0", TimeType::DurationNs);
        let custom_timeline1 = Timeline::new("my_timeline1", TimeType::DurationNs);

        // With equal row counts, priority alone decides.
        let equal = |_: &Timeline| 42_u64;

        assert_eq!(Timeline::pick_best_timeline([], equal), log_time);
        assert_eq!(Timeline::pick_best_timeline([&log_tick], equal), log_tick);
        assert_eq!(Timeline::pick_best_timeline([&log_time], equal), log_time);
        assert_eq!(
            Timeline::pick_best_timeline([&log_time, &log_tick], equal),
            log_time
        );
        assert_eq!(
            Timeline::pick_best_timeline([&log_time, &log_tick, &custom_timeline0], equal,),
            custom_timeline0
        );
        assert_eq!(
            Timeline::pick_best_timeline([&custom_timeline0, &log_time, &log_tick], equal,),
            custom_timeline0
        );
        assert_eq!(
            Timeline::pick_best_timeline([&log_time, &custom_timeline0, &log_tick], equal,),
            custom_timeline0
        );
        assert_eq!(
            Timeline::pick_best_timeline([&custom_timeline0, &log_time], equal),
            custom_timeline0
        );
        assert_eq!(
            Timeline::pick_best_timeline([&custom_timeline0, &log_tick], equal),
            custom_timeline0
        );
        assert_eq!(
            Timeline::pick_best_timeline([&log_time, &custom_timeline0], equal),
            custom_timeline0
        );
        assert_eq!(
            Timeline::pick_best_timeline([&log_tick, &custom_timeline0], equal),
            custom_timeline0
        );
        assert_eq!(
            Timeline::pick_best_timeline([&custom_timeline0], equal),
            custom_timeline0
        );

        // Row count breaks ties between timelines with the same priority.
        let more_rows_on_1 = |t: &Timeline| {
            if *t == custom_timeline1 { 100 } else { 10 }
        };
        assert_eq!(
            Timeline::pick_best_timeline([&custom_timeline0, &custom_timeline1], more_rows_on_1),
            custom_timeline1
        );
        assert_eq!(
            Timeline::pick_best_timeline([&custom_timeline1, &custom_timeline0], more_rows_on_1),
            custom_timeline1
        );

        // `message_log_time` beats all other timelines (even user-defined ones).
        let message_log_time = Timeline::new("message_log_time", TimeType::TimestampNs);
        assert_eq!(
            Timeline::pick_best_timeline([&custom_timeline0, &message_log_time, &log_time], equal),
            message_log_time
        );
        assert_eq!(
            Timeline::pick_best_timeline([&log_time, &log_tick, &message_log_time], equal),
            message_log_time
        );
    }
}
