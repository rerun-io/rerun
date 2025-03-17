use std::sync::Arc;

use arrow::datatypes::DataType as ArrowDataType;

use crate::{ResolvedTimeRange, Time, TimestampFormat};

use super::{Duration, TimeInt};

/// The type of a [`TimeInt`] or [`crate::Timeline`].
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, num_derive::FromPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum TimeType {
    /// Used e.g. for frames in a film.
    Sequence,

    /// Duration measured in nanoseconds.
    DurationNs,

    /// Nanoseconds since unix epoch (1970-01-01 00:00:00 UTC).
    TimestampNs,
}

impl std::fmt::Display for TimeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sequence => f.write_str("sequence"),
            Self::DurationNs => f.write_str("duration"),
            Self::TimestampNs => f.write_str("timestamp"),
        }
    }
}

impl TimeType {
    #[inline]
    pub(crate) fn hash(&self) -> u64 {
        match self {
            Self::Sequence => 0,
            Self::DurationNs => 1,
            Self::TimestampNs => 2,
        }
    }

    pub fn format_sequence(time_int: TimeInt) -> String {
        Self::Sequence.format(time_int, TimestampFormat::Utc)
    }

    pub fn parse_sequence(s: &str) -> Option<TimeInt> {
        match s {
            "<static>" | "static" => Some(TimeInt::STATIC),
            "−∞" | "-inf" | "-infinity" => Some(TimeInt::MIN),
            "∞" | "+∞" | "inf" | "infinity" => Some(TimeInt::MAX),
            _ => {
                let s = s.strip_prefix('#').unwrap_or(s);
                re_format::parse_i64(s).map(TimeInt::new_temporal)
            }
        }
    }

    /// Parses a human-readable time string into a [`TimeInt`].
    pub fn parse_time(&self, s: &str, timestamp_format: TimestampFormat) -> Option<TimeInt> {
        match s.to_lowercase().as_str() {
            "<static>" | "static" => Some(TimeInt::STATIC),
            "−∞" | "-inf" | "-infinity" => Some(TimeInt::MIN),
            "∞" | "+∞" | "inf" | "infinity" => Some(TimeInt::MAX),
            _ => {
                match self {
                    Self::Sequence => {
                        if let Some(s) = s.strip_prefix('#') {
                            TimeInt::try_from(re_format::parse_i64(s)?).ok()
                        } else {
                            TimeInt::try_from(re_format::parse_i64(s)?).ok()
                        }
                    }
                    Self::DurationNs => {
                        if let Some(nanos) = re_format::parse_i64(s) {
                            // If it's just numbers, interpret it as a raw nanoseconds
                            nanos.try_into().ok()
                        } else {
                            s.parse::<super::Duration>()
                                .ok()
                                .map(|duration| duration.into())
                        }
                    }
                    Self::TimestampNs => {
                        if let Some(nanos) = re_format::parse_i64(s) {
                            // If it's just numbers, interpret it as a raw nanoseconds since epoch
                            nanos.try_into().ok()
                        } else {
                            // Otherwise, try to make sense of the time string depending on the timezone setting:
                            Time::parse(s, timestamp_format)?.try_into().ok()
                        }
                    }
                }
            }
        }
    }

    pub fn format(
        &self,
        time_int: impl Into<TimeInt>,
        timestamp_format: TimestampFormat,
    ) -> String {
        let time_int = time_int.into();
        match time_int {
            TimeInt::STATIC => "<static>".into(),
            TimeInt::MIN => "−∞".into(),
            TimeInt::MAX => "+∞".into(),
            _ => match self {
                Self::Sequence => format!("#{}", re_format::format_int(time_int.as_i64())),
                Self::DurationNs => Duration::from(time_int).format_seconds(),
                Self::TimestampNs => Time::from(time_int).format(timestamp_format),
            },
        }
    }

    #[inline]
    pub fn format_utc(&self, time_int: TimeInt) -> String {
        self.format(time_int, TimestampFormat::Utc)
    }

    #[inline]
    pub fn format_range(
        &self,
        time_range: ResolvedTimeRange,
        timestamp_format: TimestampFormat,
    ) -> String {
        format!(
            "{}..={}",
            self.format(time_range.min(), timestamp_format),
            self.format(time_range.max(), timestamp_format)
        )
    }

    #[inline]
    pub fn format_range_utc(&self, time_range: ResolvedTimeRange) -> String {
        self.format_range(time_range, TimestampFormat::Utc)
    }

    /// Returns the appropriate arrow datatype to represent this timeline.
    #[inline]
    pub fn datatype(self) -> ArrowDataType {
        match self {
            Self::Sequence => ArrowDataType::Int64,
            Self::DurationNs => ArrowDataType::Duration(arrow::datatypes::TimeUnit::Nanosecond),
            Self::TimestampNs => {
                ArrowDataType::Timestamp(arrow::datatypes::TimeUnit::Nanosecond, Some("UTC".into()))
            }
        }
    }

    pub fn from_arrow_datatype(datatype: &ArrowDataType) -> Option<Self> {
        match datatype {
            ArrowDataType::Int64 => Some(Self::Sequence),
            ArrowDataType::Duration(arrow::datatypes::TimeUnit::Nanosecond) => {
                Some(Self::DurationNs)
            }
            ArrowDataType::Timestamp(arrow::datatypes::TimeUnit::Nanosecond, _) => {
                Some(Self::TimestampNs)
            }
            _ => None,
        }
    }

    /// Returns an array with the appropriate datatype.
    pub fn make_arrow_array(
        self,
        times: impl Into<arrow::buffer::ScalarBuffer<i64>>,
    ) -> arrow::array::ArrayRef {
        let times = times.into();
        match self {
            Self::Sequence => Arc::new(arrow::array::Int64Array::new(times, None)),
            Self::DurationNs => Arc::new(arrow::array::DurationNanosecondArray::new(times, None)),
            Self::TimestampNs => Arc::new(arrow::array::TimestampNanosecondArray::new(times, None)),
        }
    }

    /// Returns an array with the appropriate datatype, using `None` for [`TimeInt::STATIC`].
    pub fn make_arrow_array_from_time_ints(
        self,
        times: impl Iterator<Item = TimeInt>,
    ) -> arrow::array::ArrayRef {
        match self {
            Self::Sequence => Arc::new(
                times
                    .map(|time| {
                        if time.is_static() {
                            None
                        } else {
                            Some(time.as_i64())
                        }
                    })
                    .collect::<arrow::array::Int64Array>(),
            ),

            Self::DurationNs => Arc::new(
                times
                    .map(|time| {
                        if time.is_static() {
                            None
                        } else {
                            Some(time.as_i64())
                        }
                    })
                    .collect::<arrow::array::DurationNanosecondArray>(),
            ),

            Self::TimestampNs => Arc::new(
                times
                    .map(|time| {
                        if time.is_static() {
                            None
                        } else {
                            Some(time.as_i64())
                        }
                    })
                    .collect::<arrow::array::TimestampNanosecondArray>(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{TimeInt, TimeType};

    #[test]
    fn test_format_parse() {
        let cases = [
            (TimeInt::STATIC, "<static>"),
            (TimeInt::MIN, "−∞"),
            (TimeInt::MAX, "+∞"),
            (TimeInt::new_temporal(-42), "#−42"),
            (TimeInt::new_temporal(12345), "#12 345"),
        ];

        for (int, s) in cases {
            assert_eq!(TimeType::format_sequence(int), s);
            assert_eq!(TimeType::parse_sequence(s), Some(int));
        }
    }
}
