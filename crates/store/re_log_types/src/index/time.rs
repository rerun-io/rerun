use anyhow::Result;
use std::ops::RangeInclusive;

use crate::{Duration, Timestamp, TimestampFormat};

/// Either a [`Timestamp`] or a [`Duration`].
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Time(i64);

impl Time {
    #[inline]
    pub fn now() -> Self {
        Timestamp::now().into()
    }

    #[inline]
    pub fn nanos_since_epoch(&self) -> i64 {
        self.0
    }

    #[inline]
    pub fn from_ns_since_epoch(ns_since_epoch: i64) -> Self {
        Self(ns_since_epoch)
    }

    #[inline]
    pub fn from_us_since_epoch(us_since_epoch: i64) -> Self {
        Self(us_since_epoch * 1_000)
    }

    #[inline]
    pub fn from_seconds_since_epoch(secs: f64) -> Self {
        Self::from_ns_since_epoch((secs * 1e9).round() as _)
    }

    pub fn as_seconds_since_epoch(&self) -> f64 {
        1e-9 * self.nanos_since_epoch() as f64
    }

    /// If true, this time is likely relative to unix epoch.
    pub fn is_timestamp(&self) -> bool {
        let nanos_since_epoch = self.nanos_since_epoch();
        let years_since_epoch = nanos_since_epoch / 1_000_000_000 / 60 / 60 / 24 / 365;

        20 <= years_since_epoch && years_since_epoch <= 150
    }

    /// Formats the time as specified by ISO standard [`RFC3339`](https://www.rfc-editor.org/rfc/rfc3339.html).
    pub fn format_iso(&self) -> String {
        let nanos_since_epoch = self.nanos_since_epoch();

        if self.is_timestamp() {
            super::Timestamp::from_ns_since_epoch(nanos_since_epoch).format_iso()
        } else {
            // Relative time
            Duration::from_nanos(nanos_since_epoch).format_seconds()
        }
    }

    /// Human-readable formatting
    pub fn format(self, timestamp_format: TimestampFormat) -> String {
        let nanos_since_epoch = self.nanos_since_epoch();

        if self.is_timestamp() {
            Timestamp::from(self).format(timestamp_format)
        } else {
            // Relative time
            Duration::from_nanos(nanos_since_epoch).format_seconds()
        }
    }

    /// Best effort parsing of a string into a [`Time`] from a human readable string.
    pub fn parse(s: &str, timestamp_format: TimestampFormat) -> Option<Self> {
        #![expect(clippy::manual_map)]

        if let Ok(duration) = s.parse::<Duration>() {
            Some(Self::from_ns_since_epoch(duration.as_nanos()))
        } else if let Some(timestamp) = Timestamp::parse_with_format(s, timestamp_format) {
            Some(Self::from(timestamp))
        } else {
            None
        }
    }

    /// Useful when showing dates/times on a timeline and you want it compact.
    ///
    /// Shows dates when zoomed out, shows times when zoomed in,
    /// shows relative millisecond when really zoomed in.
    pub fn format_time_compact(&self, timestamp_format: TimestampFormat) -> String {
        let ns = self.nanos_since_epoch();
        if self.is_timestamp() {
            super::Timestamp::from_ns_since_epoch(ns).format_time_compact(timestamp_format)
        } else {
            crate::Duration::from_nanos(ns).format_subsecond_as_relative()
        }
    }

    #[inline]
    pub fn lerp(range: RangeInclusive<Self>, t: f32) -> Self {
        let (min, max) = (range.start().0, range.end().0);
        Self(min + ((max - min) as f64 * (t as f64)).round() as i64)
    }
}

impl From<Duration> for Time {
    #[inline]
    fn from(duration: Duration) -> Self {
        Self(duration.as_nanos())
    }
}

impl From<Time> for Duration {
    #[inline]
    fn from(time: Time) -> Self {
        Self::from_nanos(time.nanos_since_epoch())
    }
}

impl From<Timestamp> for Time {
    #[inline]
    fn from(timestamp: Timestamp) -> Self {
        Self(timestamp.ns_since_epoch())
    }
}

impl From<Time> for Timestamp {
    #[inline]
    fn from(time: Time) -> Self {
        Self::from_ns_since_epoch(time.nanos_since_epoch())
    }
}

impl std::fmt::Debug for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format(TimestampFormat::Utc).fmt(f)
    }
}

impl std::ops::Sub for Time {
    type Output = Duration;

    #[inline]
    fn sub(self, rhs: Self) -> Duration {
        Duration::from_nanos(self.0.saturating_sub(rhs.0))
    }
}

impl std::ops::Add<Duration> for Time {
    type Output = Self;

    #[inline]
    fn add(self, duration: Duration) -> Self::Output {
        Self(self.0.saturating_add(duration.as_nanos()))
    }
}

impl std::ops::AddAssign<Duration> for Time {
    #[inline]
    fn add_assign(&mut self, duration: Duration) {
        self.0 = self.0.saturating_add(duration.as_nanos());
    }
}

impl std::ops::Sub<Duration> for Time {
    type Output = Self;

    #[inline]
    fn sub(self, duration: Duration) -> Self::Output {
        Self(self.0.saturating_sub(duration.as_nanos()))
    }
}

impl TryFrom<std::time::SystemTime> for Time {
    type Error = std::time::SystemTimeError;

    fn try_from(time: std::time::SystemTime) -> Result<Self, Self::Error> {
        time.duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|duration_since_epoch| Self(duration_since_epoch.as_nanos() as _))
    }
}

// On non-wasm32 builds, `web_time::SystemTime` is a re-export of `std::time::SystemTime`,
// so it's covered by the above `TryFrom`.
#[cfg(target_arch = "wasm32")]
impl TryFrom<web_time::SystemTime> for Time {
    type Error = web_time::SystemTimeError;

    fn try_from(time: web_time::SystemTime) -> Result<Self, Self::Error> {
        time.duration_since(web_time::SystemTime::UNIX_EPOCH)
            .map(|duration_since_epoch| Self(duration_since_epoch.as_nanos() as _))
    }
}

// ---------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formatting_short_times() {
        assert_eq!(
            &Time::from_us_since_epoch(42_000_000).format(TimestampFormat::Utc),
            "+42s"
        );
        assert_eq!(
            &Time::from_us_since_epoch(69_000).format(TimestampFormat::Utc),
            "+0.069s"
        );
        assert_eq!(
            &Time::from_us_since_epoch(69_900).format(TimestampFormat::Utc),
            "+0.070s"
        );

        assert_eq!(
            &Time::from_us_since_epoch(42_000_000).format(TimestampFormat::LocalTimezone),
            "+42s"
        );
        assert_eq!(
            &Time::from_us_since_epoch(42_123_000_000).format(TimestampFormat::LocalTimezone),
            "+42 123s"
        );
        assert_eq!(
            &Time::from_us_since_epoch(69_000).format(TimestampFormat::LocalTimezone),
            "+0.069s"
        );
        assert_eq!(
            &Time::from_us_since_epoch(69_900).format(TimestampFormat::LocalTimezone),
            "+0.070s"
        );

        assert_eq!(
            &Time::from_us_since_epoch(42_000_000).format(TimestampFormat::UnixEpoch),
            "+42s"
        );
        assert_eq!(
            &Time::from_us_since_epoch(69_000).format(TimestampFormat::UnixEpoch),
            "+0.069s"
        );
        assert_eq!(
            &Time::from_us_since_epoch(69_900).format(TimestampFormat::UnixEpoch),
            "+0.070s"
        );
    }

    fn parse_datetime(s: &str) -> Time {
        crate::Time::parse(s, TimestampFormat::Utc).unwrap()
    }

    #[test]
    fn test_formatting_whole_second_for_datetime() {
        let datetime = parse_datetime("2022-02-28 22:35:42Z");
        assert_eq!(
            &datetime.format(TimestampFormat::Utc),
            "2022-02-28 22:35:42Z"
        );
    }

    #[test]
    fn test_formatting_whole_millisecond_for_datetime() {
        let datetime = parse_datetime("2022-02-28 22:35:42.069Z");
        assert_eq!(
            &datetime.format(TimestampFormat::Utc),
            "2022-02-28 22:35:42.069Z"
        );
    }

    #[test]
    fn test_formatting_many_digits_for_datetime() {
        let datetime = parse_datetime("2022-02-28 22:35:42.0690427Z");
        assert_eq!(
            &datetime.format(TimestampFormat::Utc),
            "2022-02-28 22:35:42.069042Z"
        ); // format function is not rounding
    }

    #[test]
    fn test_parsing_time() {
        let all_formats = [
            TimestampFormat::Utc,
            TimestampFormat::LocalTimezone,
            TimestampFormat::UnixEpoch,
        ];

        // Test relative time parsing with different units
        // Should be independent of the time zone setting.
        for format in all_formats {
            assert_eq!(
                Time::parse("42s", format),
                Some(Time::from_seconds_since_epoch(42.0))
            );
            assert_eq!(
                Time::parse("42.123s", format),
                Some(Time::from_seconds_since_epoch(42.123))
            );
            assert_eq!(
                Time::parse("42ms", format),
                Some(Time::from_seconds_since_epoch(0.042))
            );
            assert_eq!(
                Time::parse("42us", format),
                Some(Time::from_seconds_since_epoch(0.000042))
            );
            assert_eq!(
                Time::parse("42ns", format),
                Some(Time::from_seconds_since_epoch(0.000000042))
            );

            // Hour format.
            assert_eq!(
                Time::parse("22:35:42", format),
                Some(parse_datetime("1970-01-01 22:35:42Z"))
            );

            // Hour format with fractional seconds.
            assert_eq!(
                Time::parse("22:35:42.069", format),
                Some(parse_datetime("1970-01-01 22:35:42.069Z"))
            );
        }

        // Full dates.
        // Fun fact: 1954-04-11 is by some considered the least eventful day in history!
        // Full date and time
        assert_eq!(
            Time::parse("1954-04-11 22:35:42", TimestampFormat::Utc),
            Some(parse_datetime("1954-04-11 22:35:42Z"))
        );
        // Full date and time with milliseconds
        assert_eq!(
            Time::parse("1954-04-11 22:35:42.069", TimestampFormat::Utc),
            Some(parse_datetime("1954-04-11 22:35:42.069Z"))
        );
        // Timezone setting doesn't matter ifZ is enabled.
        for format in all_formats {
            // Full date and time with Z suffix
            assert_eq!(
                Time::parse("1954-04-11 22:35:42Z", format),
                Some(parse_datetime("1954-04-11 22:35:42Z"))
            );

            // Full date and time with milliseconds with Z suffix
            assert_eq!(
                Time::parse("1954-04-11 22:35:42.069Z", format),
                Some(parse_datetime("1954-04-11 22:35:42.069Z"))
            );
        }

        // Test invalid formats
        assert_eq!(Time::parse("invalid", TimestampFormat::Utc), None);
        assert_eq!(Time::parse("2022-13-28", TimestampFormat::Utc), None); // Invalid month
        assert_eq!(Time::parse("2022-02-29", TimestampFormat::Utc), None); // Invalid day (not leap year)
    }
}
