use std::ops::RangeInclusive;
use std::sync::Arc;

use arrow::array::{DurationNanosecondArray, Int64Array, TimestampNanosecondArray};
use arrow::buffer::ScalarBuffer;
use arrow::datatypes::DataType as ArrowDataType;
use arrow::error::ArrowError;
use re_arrow_util::ArrowArrayDowncastRef as _;

use super::TimeInt;
use crate::{AbsoluteTimeRange, TimestampFormat};

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

impl re_byte_size::SizeBytes for TimeType {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        0
    }
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
        Self::Sequence.format(time_int, TimestampFormat::utc())
    }

    pub fn parse_sequence(s: &str) -> Option<TimeInt> {
        match s {
            "<static>" | "static" => Some(TimeInt::STATIC),
            "beginning" | "−∞" | "-inf" | "-infinity" => Some(TimeInt::MIN),
            "end" | "∞" | "+∞" | "inf" | "infinity" => Some(TimeInt::MAX),
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
            "beginning" | "−∞" | "-inf" | "-infinity" => Some(TimeInt::MIN),
            "end" | "∞" | "+∞" | "inf" | "infinity" => Some(TimeInt::MAX),
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
                            super::Timestamp::parse_with_format(s, timestamp_format)
                                .map(|timestamp| timestamp.into())
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
        let subsecond_decimals = 0..=6; // NOTE: we currently ignore sub-microsecond
        self.format_opt(time_int, timestamp_format, subsecond_decimals)
    }

    /// The format will omit trailing sub-second zeroes as far as `subsecond_decimals` perimts it.
    pub fn format_opt(
        &self,
        time_int: impl Into<TimeInt>,
        timestamp_format: TimestampFormat,
        subsecond_decimals: RangeInclusive<usize>,
    ) -> String {
        let time_int = time_int.into();
        match time_int {
            TimeInt::STATIC => "<static>".into(),
            TimeInt::MIN => "beginning".into(),
            TimeInt::MAX => "end".into(),
            _ => match self {
                Self::Sequence => format!("#{}", re_format::format_int(time_int.as_i64())),
                Self::DurationNs => super::Duration::from(time_int).format_secs(subsecond_decimals),
                Self::TimestampNs => super::Timestamp::from(time_int)
                    .format_opt(timestamp_format, subsecond_decimals),
            },
        }
    }

    #[inline]
    pub fn format_utc(&self, time_int: TimeInt) -> String {
        self.format(time_int, TimestampFormat::utc())
    }

    #[inline]
    pub fn format_range(
        &self,
        time_range: AbsoluteTimeRange,
        timestamp_format: TimestampFormat,
    ) -> String {
        format!(
            "{}..={}",
            self.format(time_range.min(), timestamp_format),
            self.format(time_range.max(), timestamp_format)
        )
    }

    #[inline]
    pub fn format_range_utc(&self, time_range: AbsoluteTimeRange) -> String {
        self.format_range(time_range, TimestampFormat::utc())
    }

    /// Returns the appropriate arrow datatype to represent this timeline.
    #[inline]
    pub fn datatype(self) -> ArrowDataType {
        match self {
            Self::Sequence => ArrowDataType::Int64,
            Self::DurationNs => ArrowDataType::Duration(arrow::datatypes::TimeUnit::Nanosecond),
            Self::TimestampNs => {
                // TODO(zehiko) add back timezone support (#9310)
                ArrowDataType::Timestamp(arrow::datatypes::TimeUnit::Nanosecond, None)
            }
        }
    }

    pub fn from_arrow_datatype(datatype: &ArrowDataType) -> Option<Self> {
        match datatype {
            ArrowDataType::Int64 => Some(Self::Sequence),
            ArrowDataType::Duration(arrow::datatypes::TimeUnit::Nanosecond) => {
                Some(Self::DurationNs)
            }
            ArrowDataType::Timestamp(arrow::datatypes::TimeUnit::Nanosecond, timezone) => {
                // If the timezone is empty/None, it means we don't know the epoch.
                // But we will assume it's UTC anyway.
                if timezone.as_ref().is_none_or(|tz| tz.is_empty()) {
                    // TODO(#9310): warn when timezone is missing
                } else {
                    // Regardless of the timezone, that actual values are in UTC (per arrow standard)
                    // The timezone is mostly a hint on how to _display_ the time, and we currently ignore that.
                }

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
            Self::Sequence => Arc::new(Int64Array::new(times, None)),
            Self::DurationNs => Arc::new(DurationNanosecondArray::new(times, None)),
            // TODO(zehiko) add back timezone support (#9310)
            Self::TimestampNs => Arc::new(TimestampNanosecondArray::new(times, None)),
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
                    .collect::<Int64Array>(),
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
                    .collect::<DurationNanosecondArray>(),
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
                    // TODO(zehiko) add back timezone support (#9310)
                    .collect::<TimestampNanosecondArray>(),
            ),
        }
    }

    /// Take an array of time values, and based on its data type,
    /// figure out its [`TimeType`] and contents.
    pub fn from_arrow_array(
        array: &dyn arrow::array::Array,
    ) -> Result<(Self, &ScalarBuffer<i64>), ArrowError> {
        if let Some(array) = array.downcast_array_ref::<TimestampNanosecondArray>() {
            Ok((Self::TimestampNs, array.values()))
        } else if let Some(array) = array.downcast_array_ref::<DurationNanosecondArray>() {
            Ok((Self::DurationNs, array.values()))
        } else if let Some(array) = array.downcast_array_ref::<Int64Array>() {
            Ok((Self::Sequence, array.values()))
        } else {
            Err(ArrowError::SchemaError(format!(
                "Expected one of TimestampNanosecond, DurationNanosecond, Int64, got: {}",
                array.data_type()
            )))
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
            (TimeInt::MIN, "beginning"),
            (TimeInt::MAX, "end"),
            (TimeInt::new_temporal(-42), "#−42"),
            (TimeInt::new_temporal(12345), "#12 345"),
        ];

        for (int, s) in cases {
            assert_eq!(TimeType::format_sequence(int), s);
            assert_eq!(TimeType::parse_sequence(s), Some(int));
        }
    }
}
