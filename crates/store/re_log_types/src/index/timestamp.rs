use jiff::tz::TimeZone;

use super::{Duration, TimestampFormat};

/// Encodes a timestamp in nanoseconds since unix epoch.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    #[inline]
    pub fn now() -> Self {
        let ns_since_epoch = web_time::SystemTime::UNIX_EPOCH
            .elapsed()
            .expect("Expected system clock to be set to after 1970")
            .as_nanos() as _;
        Self(ns_since_epoch)
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

    #[inline]
    pub fn ns_since_epoch(self) -> i64 {
        self.0
    }
}

// ------------------------------------------
// Converters

impl Timestamp {
    pub fn to_jiff_zoned(self, timestamp_format: TimestampFormat) -> jiff::Zoned {
        let jiff = jiff::Timestamp::from(self);
        match timestamp_format {
            TimestampFormat::UnixEpoch | TimestampFormat::Utc => jiff.to_zoned(TimeZone::UTC),
            TimestampFormat::LocalTimezone => match TimeZone::try_system() {
                Ok(tz) => jiff.to_zoned(tz),
                Err(err) => {
                    re_log::warn_once!("Failed to detect system time zone: {err}");
                    jiff.to_zoned(TimeZone::UTC)
                }
            },
        }
    }
}

#[expect(clippy::fallible_impl_from)]
impl From<Timestamp> for jiff::Timestamp {
    fn from(value: Timestamp) -> Self {
        // Cannot fail - see docs for jiff::Timestamp::from_nanosecond
        #[expect(clippy::unwrap_used)]
        Self::from_nanosecond(value.ns_since_epoch() as i128).unwrap()
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
    /// RFC3339
    pub fn format_iso(self) -> String {
        jiff::Timestamp::from(self).to_string()
    }

    /// Useful when showing dates/times on a timeline and you want it compact.
    ///
    /// Shows dates when zoomed out, shows times when zoomed in,
    /// shows relative millisecond when really zoomed in.
    pub fn format_time_compact(self, timestamp_format: TimestampFormat) -> String {
        match timestamp_format {
            TimestampFormat::UnixEpoch => {
                let ns = self.ns_since_epoch();
                let fractional_ns = ns % 1_000_000_000;
                let is_whole_second = fractional_ns == 0;
                if is_whole_second {
                    re_format::format_int(ns / 1_000_000_000)
                } else {
                    // Show offset since last whole second:
                    crate::Duration::from_nanos(fractional_ns).format_subsecond_as_relative()
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
    use super::*;

    #[test]
    fn test_formatting_whole_second() {
        let timestamp: Timestamp = "2022-01-01 00:00:03Z".parse().unwrap();
        assert_eq!(timestamp.ns_since_epoch(), 1_640_995_203_000_000_000);
        assert_eq!(timestamp.format_iso(), "2022-01-01T00:00:03Z");
        assert_eq!(
            "2022-01-01T00:00:03Z".parse::<Timestamp>().unwrap(),
            timestamp
        );
    }

    #[test]
    fn test_formatting_subsecond() {
        let timestamp: Timestamp = "2022-01-01 00:00:03.123456789Z".parse().unwrap();
        assert_eq!(timestamp.ns_since_epoch(), 1_640_995_203_123_456_789);
        assert_eq!(timestamp.format_iso(), "2022-01-01T00:00:03.123456789Z");
        assert_eq!(
            "2022-01-01T00:00:03.123456789Z"
                .parse::<Timestamp>()
                .unwrap(),
            timestamp
        );
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
}
