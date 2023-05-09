use arrow2::datatypes::{DataType, TimeUnit};

use crate::{SizeBytes, TimeRange, TimeType};

re_string_interner::declare_new_type!(
    /// The name of a timeline. Often something like `"log_time"` or `"frame_nr"`.
    pub struct TimelineName;
);

impl Default for TimelineName {
    fn default() -> Self {
        Self::from(String::default())
    }
}

// ----------------------------------------------------------------------------

/// A time frame/space, e.g. `log_time` or `frame_nr`, coupled with the type of time
/// it keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Timeline {
    /// Name of the timeline (e.g. "log_time").
    name: TimelineName,

    /// Sequence or time?
    typ: TimeType,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            name: TimelineName::default(),
            typ: TimeType::Sequence,
        }
    }
}

impl Timeline {
    #[inline]
    pub fn new(name: impl Into<TimelineName>, typ: TimeType) -> Self {
        Self {
            name: name.into(),
            typ,
        }
    }

    #[inline]
    pub fn new_temporal(name: impl Into<TimelineName>) -> Self {
        Self {
            name: name.into(),
            typ: TimeType::Time,
        }
    }

    #[inline]
    pub fn new_sequence(name: impl Into<TimelineName>) -> Self {
        Self {
            name: name.into(),
            typ: TimeType::Sequence,
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
        Timeline::new("log_time", TimeType::Time)
    }

    /// The log tick timeline to which all API functions will always log.
    ///
    /// This timeline is automatically maintained by the SDKs and captures the logging tick at
    /// which point the data was logged.
    /// The logging tick is monotically incremented each time the client calls one of the logging
    /// methods on a `RecordingStream`.
    #[inline]
    pub fn log_tick() -> Self {
        Timeline::new("log_tick", TimeType::Sequence)
    }

    /// Returns a formatted string of `time_range` on this `Timeline`.
    #[inline]
    pub fn format_time_range(&self, time_range: &TimeRange) -> String {
        format!(
            "    - {}: from {} to {} (all inclusive)",
            self.name,
            self.typ.format(time_range.min),
            self.typ.format(time_range.max),
        )
    }

    /// Returns the appropriate arrow datatype to represent this timeline.
    #[inline]
    pub fn datatype(&self) -> DataType {
        match self.typ {
            TimeType::Time => DataType::Timestamp(TimeUnit::Nanosecond, None),
            TimeType::Sequence => DataType::Int64,
        }
    }
}

impl nohash_hasher::IsEnabled for Timeline {}

impl SizeBytes for Timeline {
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
        state.write_u64(self.name.hash() | self.typ.hash());
    }
}
