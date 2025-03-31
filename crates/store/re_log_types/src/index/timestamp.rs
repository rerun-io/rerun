use crate::external::re_types_core;
use std::str::FromStr as _;

use super::{Duration, TimestampFormat};

/// Encodes a timestamp in nanoseconds since unix epoch.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    #[inline]
    pub fn now() -> Self {
        let nanos_since_epoch = web_time::SystemTime::UNIX_EPOCH
            .elapsed()
            .expect("Expected system clock to be set to after 1970")
            .as_nanos() as _;
        Self(nanos_since_epoch)
    }

    #[inline]
    pub fn from_nanos_since_epoch(nanos_since_epoch: i64) -> Self {
        Self(nanos_since_epoch)
    }

    #[inline]
    pub fn from_us_since_epoch(us_since_epoch: i64) -> Self {
        Self(us_since_epoch * 1_000)
    }

    #[inline]
    pub fn from_secs_since_epoch(secs: f64) -> Self {
        Self::from_nanos_since_epoch((secs * 1e9).round() as _)
    }

    #[inline]
    pub fn nanos_since_epoch(self) -> i64 {
        self.0
    }
}

// ------------------------------------------
// Rerun types converters

impl From<re_types_core::datatypes::TimeInt> for Timestamp {
    fn from(time_int: re_types_core::datatypes::TimeInt) -> Self {
        Self(time_int.0)
    }
}

// ------------------------------------------
// System converters

impl From<super::TimeInt> for Timestamp {
    #[inline]
    fn from(int: super::TimeInt) -> Self {
        Self::from_nanos_since_epoch(int.as_i64())
    }
}

impl From<Timestamp> for super::TimeInt {
    #[inline]
    fn from(timestamp: Timestamp) -> Self {
        Self::saturated_temporal_i64(timestamp.nanos_since_epoch())
    }
}

impl TryFrom<std::time::SystemTime> for Timestamp {
    type Error = std::time::SystemTimeError;

    fn try_from(time: std::time::SystemTime) -> Result<Self, Self::Error> {
        time.duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|duration_since_epoch| Self(duration_since_epoch.as_nanos() as _))
    }
}

// On non-wasm32 builds, `web_time::SystemTime` is a re-export of `std::time::SystemTime`,
// so it's covered by the above `TryFrom`.
#[cfg(target_arch = "wasm32")]
impl TryFrom<web_time::SystemTime> for Timestamp {
    type Error = web_time::SystemTimeError;

    fn try_from(time: web_time::SystemTime) -> Result<Self, Self::Error> {
        time.duration_since(web_time::SystemTime::UNIX_EPOCH)
            .map(|duration_since_epoch| Self(duration_since_epoch.as_nanos() as _))
    }
}

// ------------------------------------------
// `jiff` converters

impl Timestamp {
    pub fn to_jiff_zoned(self, timestamp_format: TimestampFormat) -> jiff::Zoned {
        jiff::Timestamp::from(self).to_zoned(timestamp_format.to_jiff_time_zone())
    }
}

#[expect(clippy::fallible_impl_from)]
impl From<Timestamp> for jiff::Timestamp {
    fn from(value: Timestamp) -> Self {
        // Cannot fail - see docs for jiff::Timestamp::from_nanosecond
        #[expect(clippy::unwrap_used)]
        Self::from_nanosecond(value.nanos_since_epoch() as i128).unwrap()
    }
}

impl From<jiff::Timestamp> for Timestamp {
    fn from(value: jiff::Timestamp) -> Self {
        Self(value.as_nanosecond() as i64)
    }
}

impl From<jiff::Zoned> for Timestamp {
    fn from(value: jiff::Zoned) -> Self {
        value.timestamp().into()
    }
}

// ------------------------------------------
// Formatting and parsing

impl std::str::FromStr for Timestamp {
    type Err = jiff::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let jiff_timestamp = jiff::Timestamp::from_str(s)?;
        Ok(Self(jiff_timestamp.as_nanosecond() as i64))
    }
}

impl std::fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format_iso().fmt(f)
    }
}

impl Timestamp {
    /// Formats the time as specified by ISO standard [`RFC3339`](https://www.rfc-editor.org/rfc/rfc3339.html).
    pub fn format_iso(self) -> String {
        jiff::Timestamp::from(self).to_string()
    }

    /// Human-readable timestamp.
    ///
    /// Omits the date of same-day timestamps.
    pub fn format(self, timestamp_format: TimestampFormat) -> String {
        let format_fractional_nanos = |ns: i32| {
            let is_whole_sec = ns % 1_000_000_000 == 0;
            let is_whole_ms = ns % 1_000_000 == 0;

            if is_whole_sec {
                String::new()
            } else if is_whole_ms {
                format!(".{:03}", ns / 1_000_000)
            } else {
                // NOTE: we currently ignore sub-microsecond
                format!(".{:06}", ns / 1_000)
            }
        };

        let timestamp = jiff::Timestamp::from(self);

        match timestamp_format {
            TimestampFormat::UnixEpoch => {
                format!(
                    "{}{}",
                    timestamp.as_second(),
                    format_fractional_nanos(timestamp.subsec_nanosecond())
                )
            }

            TimestampFormat::LocalTimezone | TimestampFormat::Utc => {
                let tz = timestamp_format.to_jiff_time_zone();
                let zoned = timestamp.to_zoned(tz.clone());

                let is_today = zoned.date() == jiff::Timestamp::now().to_zoned(tz).date();

                let formatted = if is_today {
                    zoned.strftime("%H:%M:%S").to_string()
                } else {
                    zoned.strftime("%Y-%m-%d %H:%M:%S").to_string()
                };

                let suffix = match timestamp_format {
                    TimestampFormat::LocalTimezone => "",
                    TimestampFormat::Utc | TimestampFormat::UnixEpoch => "Z",
                };

                format!(
                    "{formatted}{}{suffix}",
                    format_fractional_nanos(zoned.subsec_nanosecond())
                )
            }
        }
    }

    /// Useful when showing dates/times on a timeline and you want it compact.
    ///
    /// Shows dates when zoomed out, shows times when zoomed in,
    /// shows relative millisecond when really zoomed in.
    pub fn format_time_compact(self, timestamp_format: TimestampFormat) -> String {
        match timestamp_format {
            TimestampFormat::UnixEpoch => {
                let ns = self.nanos_since_epoch();
                let fractional_nanos = ns % 1_000_000_000;
                let is_whole_second = fractional_nanos == 0;
                if is_whole_second {
                    re_format::format_int(ns / 1_000_000_000)
                } else {
                    // Show offset since last whole second:
                    crate::Duration::from_nanos(fractional_nanos).format_subsecond_as_relative()
                }
            }

            TimestampFormat::LocalTimezone | TimestampFormat::Utc => {
                let zoned = self.to_jiff_zoned(timestamp_format);
                if zoned.time() == jiff::civil::Time::MIN {
                    // Exactly midnight - show only the date:
                    zoned.strftime("%Y-%m-%d").to_string()
                } else if zoned.subsec_nanosecond() != 0 {
                    // Show offset since last whole second:
                    crate::Duration::from_nanos(zoned.subsec_nanosecond() as _)
                        .format_subsecond_as_relative()
                } else if zoned.second() == 0 {
                    zoned.strftime("%H:%M").to_string()
                } else {
                    zoned.strftime("%H:%M:%S").to_string()
                }
            }
        }
    }

    /// Parse a timestamp,
    ///
    /// If it is missing a timezone specifier, the given timezone is assumed.
    pub fn parse_with_format(s: &str, timestamp_format: TimestampFormat) -> Option<Self> {
        if let Ok(utc) = Self::from_str(s) {
            // It has a `Z` suffix
            Some(utc)
        } else if let Ok(zoned) = jiff::Zoned::from_str(s) {
            // It had a timezone suffix
            Some(Self::from(zoned))
        } else if let Ok(date_time) = jiff::civil::DateTime::from_str(s) {
            date_time
                .to_zoned(timestamp_format.to_jiff_time_zone())
                .ok()
                .map(|zoned| zoned.into())
        } else if timestamp_format == TimestampFormat::UnixEpoch {
            let ns = re_format::parse_i64(s)?;
            Some(Self::from_nanos_since_epoch(ns))
        } else {
            None
        }
    }
}

// ------------------------------------------
// Duration ops

impl std::ops::Sub for Timestamp {
    type Output = Duration;

    #[inline]
    fn sub(self, rhs: Self) -> Duration {
        Duration::from_nanos(self.0.saturating_sub(rhs.0))
    }
}

impl std::ops::Add<Duration> for Timestamp {
    type Output = Self;

    #[inline]
    fn add(self, duration: Duration) -> Self::Output {
        Self(self.0.saturating_add(duration.as_nanos()))
    }
}

impl std::ops::AddAssign<Duration> for Timestamp {
    #[inline]
    fn add_assign(&mut self, duration: Duration) {
        self.0 = self.0.saturating_add(duration.as_nanos());
    }
}

impl std::ops::Sub<Duration> for Timestamp {
    type Output = Self;

    #[inline]
    fn sub(self, duration: Duration) -> Self::Output {
        Self(self.0.saturating_sub(duration.as_nanos()))
    }
}

// ---------------------------------------------

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use super::*;

    #[test]
    fn test_formatting_whole_second() {
        let timestamp: Timestamp = "2022-01-01 00:00:03Z".parse().unwrap();
        assert_eq!(timestamp.nanos_since_epoch(), 1_640_995_203_000_000_000);
        assert_eq!(timestamp.format_iso(), "2022-01-01T00:00:03Z");
        assert_eq!(
            "2022-01-01T00:00:03Z".parse::<Timestamp>().unwrap(),
            timestamp
        );
    }

    #[test]
    fn test_formatting_subsecond() {
        let timestamp: Timestamp = "2022-01-01 00:00:03.123456789Z".parse().unwrap();
        assert_eq!(timestamp.nanos_since_epoch(), 1_640_995_203_123_456_789);
        assert_eq!(timestamp.format_iso(), "2022-01-01T00:00:03.123456789Z");
        assert_eq!(
            "2022-01-01T00:00:03.123456789Z"
                .parse::<Timestamp>()
                .unwrap(),
            timestamp
        );
    }

    #[test]
    fn test_formatting_whole_second_for_datetime() {
        let datetime = Timestamp::from_str("2022-02-28 22:35:42Z").unwrap();
        assert_eq!(
            &datetime.format(TimestampFormat::Utc),
            "2022-02-28 22:35:42Z"
        );
    }

    #[test]
    fn test_formatting_whole_millisecond_for_datetime() {
        let datetime = Timestamp::from_str("2022-02-28 22:35:42.069Z").unwrap();
        assert_eq!(
            &datetime.format(TimestampFormat::Utc),
            "2022-02-28 22:35:42.069Z"
        );
    }

    #[test]
    fn test_formatting_many_digits_for_datetime() {
        let datetime = Timestamp::from_str("2022-02-28 22:35:42.0690427Z").unwrap();
        assert_eq!(
            &datetime.format(TimestampFormat::Utc),
            "2022-02-28 22:35:42.069042Z"
        ); // format function is not rounding
    }

    /// Check that formatting today times doesn't display the date.
    /// WARNING: this test could flake if run on midnight
    #[test]
    fn test_formatting_today_omit_date() {
        let tz = jiff::tz::TimeZone::UTC;
        let today = jiff::Timestamp::now()
            .to_zoned(tz)
            .with()
            .time(jiff::civil::Time::new(22, 35, 42, 0).unwrap())
            .build()
            .unwrap();
        let datetime = Timestamp::from(today);
        assert_eq!(&datetime.format(TimestampFormat::Utc), "22:35:42Z");
    }

    #[test]
    fn test_format_compact() {
        for (input, expected) in [
            ("2022-01-01T01:02:03.12345Z", "+123.45 ms"),
            ("2022-01-01T01:02:03.123Z", "+123 ms"),
            ("2022-01-01T01:02:03Z", "01:02:03"),
            ("2022-01-01T01:02:00Z", "01:02"),
            ("2022-01-01T00:00:00Z", "2022-01-01"),
        ] {
            let timestamp: Timestamp = input.parse().unwrap();
            let formatted = timestamp.format_time_compact(TimestampFormat::Utc);
            assert_eq!(formatted, expected);
        }
    }

    #[test]
    fn test_parsing_timestamp() {
        fn parse(s: &str, format: TimestampFormat) -> Option<Timestamp> {
            Timestamp::parse_with_format(s, format)
        }

        let all_formats = [
            TimestampFormat::Utc,
            TimestampFormat::LocalTimezone,
            TimestampFormat::UnixEpoch,
        ];

        // Full dates.
        // Fun fact: 1954-04-11 is by some considered the least eventful day in history!
        // Full date and time
        assert_eq!(
            parse("1954-04-11 22:35:42", TimestampFormat::Utc),
            Some(Timestamp::from_str("1954-04-11 22:35:42Z").unwrap())
        );
        // Full date and time with milliseconds
        assert_eq!(
            parse("1954-04-11 22:35:42.069", TimestampFormat::Utc),
            Some(Timestamp::from_str("1954-04-11 22:35:42.069Z").unwrap())
        );

        // Timezone setting doesn't matter if UTC is enabled.
        for format in all_formats {
            // Full date and time with Z suffix
            assert_eq!(
                parse("1954-04-11T22:35:42Z", format),
                Some(Timestamp::from_str("1954-04-11 22:35:42Z").unwrap()),
                "Failed for format {format:?}"
            );

            // Full date and time with milliseconds with Z suffix
            assert_eq!(
                parse("1954-04-11 22:35:42.069Z", format),
                Some(Timestamp::from_str("1954-04-11 22:35:42.069Z").unwrap())
            );
        }

        // Current timezone.
        // Full date and time.
        if let Ok(tz) = jiff::tz::TimeZone::try_system() {
            assert_eq!(
                parse("1954-04-11 22:35:42", TimestampFormat::LocalTimezone),
                Some(Timestamp::from(
                    jiff::civil::DateTime::from_str("1954-04-11 22:35:42")
                        .unwrap()
                        .to_zoned(tz.clone())
                        .unwrap()
                ))
            );
            // Full date and time with milliseconds
            assert_eq!(
                parse("1954-04-11 22:35:42.069", TimestampFormat::LocalTimezone),
                Some(Timestamp::from(
                    jiff::civil::DateTime::from_str("1954-04-11 22:35:42.069")
                        .unwrap()
                        .to_zoned(tz)
                        .unwrap()
                ))
            );
        }

        // Test invalid formats
        assert_eq!(parse("invalid", TimestampFormat::Utc), None);
        assert_eq!(parse("2022-13-28", TimestampFormat::Utc), None); // Invalid month
        assert_eq!(parse("2022-02-29", TimestampFormat::Utc), None); // Invalid day (not leap year)
    }
}
