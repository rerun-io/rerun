use crate::{ResolvedTimeRange, TimeType, TimestampFormat};

re_string_interner::declare_new_type!(
    /// The name of a timeline. Often something like `"log_time"` or `"frame_nr"`.
    ///
    /// This uniquely identifies a timeline.
    #[cfg_attr(feature = "serde", derive(::serde::Deserialize, ::serde::Serialize))]
    pub struct TimelineName;
);

impl TimelineName {
    /// The log time timeline to which all API functions will always log.
    ///
    /// This timeline is automatically maintained by the SDKs and captures the wall-clock time at
    /// which point the data was logged (according to the client's wall-clock).
    #[inline]
    pub fn log_time() -> Self {
        Self::new("log_time")
    }

    /// The log tick timeline to which all API functions will always log.
    ///
    /// This timeline is automatically maintained by the SDKs and captures the logging tick at
    /// which point the data was logged.
    /// The logging tick is monotically incremented each time the client calls one of the logging
    /// methods on a `RecordingStream`.
    #[inline]
    pub fn log_tick() -> Self {
        Self::new("log_tick")
    }
}

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

    #[deprecated(
        since = "0.23.0",
        note = "Use `Timeline::new_duration` or `new_timestamp` instead"
    )]
    #[inline]
    pub fn new_temporal(name: impl Into<TimelineName>) -> Self {
        Self::new_duration(name)
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
        time_range: &ResolvedTimeRange,
        timestamp_format: TimestampFormat,
    ) -> String {
        self.typ.format_range(*time_range, timestamp_format)
    }

    /// Returns a formatted string of `time_range` on this `Timeline`.
    #[inline]
    pub fn format_time_range_utc(&self, time_range: &ResolvedTimeRange) -> String {
        self.format_time_range(time_range, TimestampFormat::Utc)
    }

    /// Returns the appropriate arrow datatype to represent this timeline.
    #[inline]
    pub fn datatype(&self) -> arrow::datatypes::DataType {
        self.typ.datatype()
    }
}

impl nohash_hasher::IsEnabled for Timeline {}

impl re_byte_size::SizeBytes for TimelineName {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

impl re_byte_size::SizeBytes for Timeline {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
}

// required for [`nohash_hasher`].
#[allow(clippy::derived_hash_with_manual_eq)]
impl std::hash::Hash for Timeline {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_u64(self.name.hash() ^ self.typ.hash());
    }
}
