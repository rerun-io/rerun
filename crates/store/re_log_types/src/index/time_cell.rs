use std::str::FromStr;

use crate::{NonMinI64, TimeInt, TimeType};

pub struct OutOfRange;

/// An typed cell of an index, e.g. a point in time on some unknown timeline.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct TimeCell {
    pub typ: TimeType,
    pub value: NonMinI64,
}

// We shouldn't implement display for this as it's too ambiguous, instead create specific functions.
static_assertions::assert_not_impl_any!(TimeCell: std::fmt::Display);

impl TimeCell {
    pub const ZERO_DURATION: Self = Self {
        typ: TimeType::DurationNs,
        value: NonMinI64::ZERO,
    };

    pub const ZERO_SEQUENCE: Self = Self {
        typ: TimeType::Sequence,
        value: NonMinI64::ZERO,
    };

    #[inline]
    pub fn new(typ: TimeType, value: impl TryInto<NonMinI64>) -> Self {
        let value = value.try_into().unwrap_or(NonMinI64::MIN); // clamp to valid range
        Self { typ, value }
    }

    #[inline]
    pub fn from_sequence(sequence: impl TryInto<NonMinI64>) -> Self {
        Self::new(TimeType::Sequence, sequence)
    }

    #[inline]
    pub fn from_duration_nanos(nanos: impl TryInto<NonMinI64>) -> Self {
        Self::new(TimeType::DurationNs, nanos)
    }

    /// Create a timestamp from number of nanoseconds since the unix epoch, 1970-01-01 00:00:00 UTC.
    #[inline]
    pub fn from_timestamp_nanos_since_epoch(nanos_since_epoch: impl TryInto<NonMinI64>) -> Self {
        Self::new(TimeType::TimestampNs, nanos_since_epoch)
    }

    /// Create a timestamp from number of seconds since the unix epoch, 1970-01-01 00:00:00 UTC.
    #[inline]
    pub fn from_timestamp_secs_since_epoch(secs_since_epoch: f64) -> Self {
        Self::from_timestamp_nanos_since_epoch((1e9 * secs_since_epoch).round() as i64)
    }

    /// A timestamp of the current clock time.
    pub fn timestamp_now() -> Self {
        crate::Timestamp::now().into()
    }

    #[inline]
    pub fn typ(&self) -> TimeType {
        self.typ
    }

    /// Internal encoding.
    ///
    /// Its meaning depends on the [`Self::typ`].
    #[inline]
    pub fn as_i64(&self) -> i64 {
        self.value.into()
    }
}

impl re_byte_size::SizeBytes for TimeCell {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }

    #[inline]
    fn is_pod() -> bool {
        true
    }
}

impl From<TimeCell> for TimeInt {
    #[inline]
    fn from(cell: TimeCell) -> Self {
        Self::from(cell.value)
    }
}

impl From<TimeCell> for NonMinI64 {
    #[inline]
    fn from(cell: TimeCell) -> Self {
        cell.value
    }
}

impl From<TimeCell> for i64 {
    #[inline]
    fn from(cell: TimeCell) -> Self {
        cell.value.get()
    }
}

// ------------------------------------------------------------------

impl From<std::time::Duration> for TimeCell {
    /// Saturating cast from [`std::time::Duration`].
    fn from(time: std::time::Duration) -> Self {
        Self::from_duration_nanos(NonMinI64::saturating_from_u128(time.as_nanos()))
    }
}

impl From<super::Duration> for TimeCell {
    #[inline]
    fn from(duration: super::Duration) -> Self {
        Self::from_duration_nanos(duration.as_nanos())
    }
}

impl From<super::Timestamp> for TimeCell {
    #[inline]
    fn from(timestamp: super::Timestamp) -> Self {
        Self::from_timestamp_nanos_since_epoch(timestamp.nanos_since_epoch())
    }
}

// ------------------------------------------------------------------

impl TryFrom<std::time::SystemTime> for TimeCell {
    type Error = OutOfRange;

    fn try_from(time: std::time::SystemTime) -> Result<Self, Self::Error> {
        let duration_since_epoch = time
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map_err(|_ignored| OutOfRange)?;
        let nanos_since_epoch = duration_since_epoch.as_nanos();
        let nanos_since_epoch = i64::try_from(nanos_since_epoch).map_err(|_ignored| OutOfRange)?;
        Ok(Self::from_timestamp_nanos_since_epoch(nanos_since_epoch))
    }
}

// On non-wasm32 builds, `web_time::SystemTime` is a re-export of `std::time::SystemTime`,
// so it's covered by the above `TryFrom`.
#[cfg(target_arch = "wasm32")]
impl TryFrom<web_time::SystemTime> for TimeCell {
    type Error = OutOfRange;

    fn try_from(time: web_time::SystemTime) -> Result<Self, Self::Error> {
        let duration_since_epoch = time
            .duration_since(web_time::SystemTime::UNIX_EPOCH)
            .map_err(|_ignored| OutOfRange)?;
        let nanos_since_epoch = duration_since_epoch.as_nanos();
        let nanos_since_epoch = i64::try_from(nanos_since_epoch).map_err(|_ignored| OutOfRange)?;
        Ok(Self::from_timestamp_nanos_since_epoch(nanos_since_epoch))
    }
}

// ------------------------------------------------------------------

impl TimeCell {
    pub fn format_compact(&self, timestamp_format: super::TimestampFormat) -> String {
        let Self { typ, value } = *self;

        match typ {
            TimeType::DurationNs => {
                crate::Duration::from_nanos(value.into()).format_subsecond_as_relative()
            }

            TimeType::TimestampNs => crate::Timestamp::from_nanos_since_epoch(value.into())
                .format_time_compact(timestamp_format),

            TimeType::Sequence => typ.format(value, timestamp_format),
        }
    }

    pub fn format(&self, timestamp_format: super::TimestampFormat) -> String {
        let Self { typ, value } = *self;

        match typ {
            TimeType::DurationNs => {
                crate::Duration::from_nanos(value.into()).format_subsecond_as_relative()
            }

            TimeType::TimestampNs => {
                crate::Timestamp::from_nanos_since_epoch(value.into()).format(timestamp_format)
            }

            TimeType::Sequence => typ.format(value, timestamp_format),
        }
    }

    /// Special format which avoids forbidden & special characters in a url.
    pub fn format_url(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Display as _;
        match self.typ {
            // NOTE: we avoid special characters here so we can put these formats in an URI
            TimeType::Sequence => write!(f, "{}", self.value),
            TimeType::DurationNs => crate::Duration::from_nanos(self.value.get()).format_url(f),
            TimeType::TimestampNs => crate::Timestamp::from_nanos_since_epoch(self.value.get())
                .format_iso()
                .fmt(f),
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("Invalid TimeCell: {0}")]
pub struct InvalidTimeCell(String);

impl FromStr for TimeCell {
    type Err = InvalidTimeCell;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(int) = NonMinI64::from_str(s) {
            Ok(Self::new(TimeType::Sequence, int))
        } else if let Ok(duration) = s.parse::<super::Duration>() {
            Ok(Self::from(duration))
        } else if let Ok(timestamp) = s.parse::<super::Timestamp>() {
            Ok(Self::from(timestamp))
        } else {
            Err(InvalidTimeCell(format!(
                "Expected a #sequence, duration, or RFC3339 timestamp, but got '{s}'"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_cell_format_and_parse() {
        let test_cases = [
            ("1234", TimeCell::from_sequence(1234)),
            ("10.134567s", TimeCell::from_duration_nanos(10_134_567_000)),
            (
                "2022-01-01T00:00:03.123456789Z",
                TimeCell::from_timestamp_nanos_since_epoch(1_640_995_203_123_456_789),
            ),
        ];

        struct UrlFormat(TimeCell);

        impl std::fmt::Display for UrlFormat {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.format_url(f)
            }
        }

        for (string, cell) in test_cases {
            assert_eq!(TimeCell::from_str(string).unwrap(), cell);
            assert_eq!(
                TimeCell::from_str(&UrlFormat(cell).to_string()).unwrap(),
                cell
            );
        }
    }
}
