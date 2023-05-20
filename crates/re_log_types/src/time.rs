use std::ops::RangeInclusive;
use time::OffsetDateTime;

/// A date-time represented as nanoseconds since unix epoch
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Time(i64);

impl Time {
    #[inline]
    pub fn now() -> Self {
        let nanos_since_epoch = web_time::SystemTime::UNIX_EPOCH
            .elapsed()
            .expect("Expected system clock to be set to after 1970")
            .as_nanos() as _;
        Self(nanos_since_epoch)
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

    /// If true, this time is likely relative to unix epoch.
    pub fn is_absolute_date(&self) -> bool {
        let nanos_since_epoch = self.nanos_since_epoch();
        let years_since_epoch = nanos_since_epoch / 1_000_000_000 / 60 / 60 / 24 / 365;

        20 <= years_since_epoch && years_since_epoch <= 150
    }

    /// Returns the absolute datetime if applicable.
    pub fn to_datetime(&self) -> Option<OffsetDateTime> {
        let ns_since_epoch = self.nanos_since_epoch();
        if self.is_absolute_date() {
            OffsetDateTime::from_unix_timestamp_nanos(ns_since_epoch as i128).ok()
        } else {
            None
        }
    }

    pub fn is_exactly_midnight(&self) -> bool {
        // This is correct despite leap seconds because
        // during positive leap seconds, UTC actually has a discontinuity
        // (the same integer is reused for two different times).
        // See https://en.wikipedia.org/wiki/Unix_time#Leap_seconds
        self.nanos_since_epoch() % (24 * 60 * 60 * 1_000_000_000) == 0
    }

    /// Human-readable formatting
    pub fn format(&self) -> String {
        let nanos_since_epoch = self.nanos_since_epoch();

        if let Some(datetime) = self.to_datetime() {
            let is_whole_second = nanos_since_epoch % 1_000_000_000 == 0;
            let is_whole_millisecond = nanos_since_epoch % 1_000_000 == 0;

            let time_format = if is_whole_second {
                "[hour]:[minute]:[second]Z"
            } else if is_whole_millisecond {
                "[hour]:[minute]:[second].[subsecond digits:3]Z"
            } else {
                "[hour]:[minute]:[second].[subsecond digits:6]Z"
            };

            let date_is_today = datetime.date() == OffsetDateTime::now_utc().date();
            let date_format = format!("[year]-[month]-[day] {time_format}");
            let parsed_format = if date_is_today {
                time::format_description::parse(time_format).unwrap()
            } else {
                time::format_description::parse(&date_format).unwrap()
            };
            datetime.format(&parsed_format).unwrap()
        } else {
            // Relative time
            let secs = nanos_since_epoch as f64 * 1e-9;

            let is_whole_second = nanos_since_epoch % 1_000_000_000 == 0;
            if is_whole_second {
                format!("{secs:+.0}s")
            } else {
                format!("{secs:+.3}s")
            }
        }
    }

    #[inline]
    pub fn lerp(range: RangeInclusive<Time>, t: f32) -> Time {
        let (min, max) = (range.start().0, range.end().0);
        Self(min + ((max - min) as f64 * (t as f64)).round() as i64)
    }
}

impl std::fmt::Debug for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.format().fmt(f)
    }
}

impl std::ops::Sub for Time {
    type Output = Duration;

    #[inline]
    fn sub(self, rhs: Time) -> Duration {
        Duration(self.0.saturating_sub(rhs.0))
    }
}

impl std::ops::Add<Duration> for Time {
    type Output = Time;

    #[inline]
    fn add(self, duration: Duration) -> Self::Output {
        Time(self.0.saturating_add(duration.0))
    }
}

impl std::ops::AddAssign<Duration> for Time {
    #[inline]
    fn add_assign(&mut self, duration: Duration) {
        self.0 = self.0.saturating_add(duration.0);
    }
}

impl std::ops::Sub<Duration> for Time {
    type Output = Time;

    #[inline]
    fn sub(self, duration: Duration) -> Self::Output {
        Time(self.0.saturating_sub(duration.0))
    }
}

impl TryFrom<std::time::SystemTime> for Time {
    type Error = std::time::SystemTimeError;

    fn try_from(time: std::time::SystemTime) -> Result<Time, Self::Error> {
        time.duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|duration_since_epoch| Time(duration_since_epoch.as_nanos() as _))
    }
}

// On non-wasm32 builds, `web_time::SystemTime` is a re-export of `std::time::SystemTime`,
// so it's covered by the above `TryFrom`.
#[cfg(target_arch = "wasm32")]
impl TryFrom<web_time::SystemTime> for Time {
    type Error = web_time::SystemTimeError;

    fn try_from(time: web_time::SystemTime) -> Result<Time, Self::Error> {
        time.duration_since(web_time::SystemTime::UNIX_EPOCH)
            .map(|duration_since_epoch| Time(duration_since_epoch.as_nanos() as _))
    }
}

impl TryFrom<time::OffsetDateTime> for Time {
    type Error = core::num::TryFromIntError;

    fn try_from(datetime: time::OffsetDateTime) -> Result<Time, Self::Error> {
        i64::try_from(datetime.unix_timestamp_nanos()).map(Time::from_ns_since_epoch)
    }
}

// ---------------

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::{datetime, time};

    #[test]
    fn test_formatting_short_times() {
        assert_eq!(&Time::from_us_since_epoch(42_000_000).format(), "+42s");
        assert_eq!(&Time::from_us_since_epoch(69_000).format(), "+0.069s");
        assert_eq!(&Time::from_us_since_epoch(69_900).format(), "+0.070s");
    }

    #[test]
    fn test_formatting_whole_second_for_datetime() {
        let datetime = Time::try_from(datetime!(2022-02-28 22:35:42 UTC)).unwrap();
        assert_eq!(&datetime.format(), "2022-02-28 22:35:42Z");
    }

    #[test]
    fn test_formatting_whole_millisecond_for_datetime() {
        let datetime = Time::try_from(datetime!(2022-02-28 22:35:42.069 UTC)).unwrap();
        assert_eq!(&datetime.format(), "2022-02-28 22:35:42.069Z");
    }

    #[test]
    fn test_formatting_many_digits_for_datetime() {
        let datetime = Time::try_from(datetime!(2022-02-28 22:35:42.069_042_7 UTC)).unwrap();
        assert_eq!(&datetime.format(), "2022-02-28 22:35:42.069042Z"); // format function is not rounding
    }

    /// Check that formatting today times doesn't display the date.
    /// WARNING: this test could easily flake with current implementation
    /// (checking day instead of hour-distance)
    #[test]
    fn test_formatting_today_omit_date() {
        let today = OffsetDateTime::now_utc().replace_time(time!(22:35:42));
        let datetime = Time::try_from(today).unwrap();
        assert_eq!(&datetime.format(), "22:35:42Z");
    }
}

// ----------------------------------------------------------------------------

/// A signed duration represented as nanoseconds since unix epoch
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Duration(i64);

impl Duration {
    pub const MAX: Duration = Duration(std::i64::MAX);
    const NANOS_PER_SEC: i64 = 1_000_000_000;
    const NANOS_PER_MILLI: i64 = 1_000_000;
    const SEC_PER_MINUTE: i64 = 60;
    const SEC_PER_HOUR: i64 = 60 * Self::SEC_PER_MINUTE;
    const SEC_PER_DAY: i64 = 24 * Self::SEC_PER_HOUR;

    #[inline]
    pub fn from_nanos(nanos: i64) -> Self {
        Self(nanos)
    }

    #[inline]
    pub fn from_millis(millis: i64) -> Self {
        Self(millis * Self::NANOS_PER_MILLI)
    }

    #[inline]
    pub fn from_secs(secs: f32) -> Self {
        Self::from_nanos((secs * Self::NANOS_PER_SEC as f32).round() as _)
    }

    #[inline]
    pub fn as_nanos(&self) -> i64 {
        self.0
    }

    #[inline]
    pub fn as_secs_f32(&self) -> f32 {
        self.0 as f32 * 1e-9
    }

    #[inline]
    pub fn as_secs_f64(&self) -> f64 {
        self.0 as f64 * 1e-9
    }

    pub fn exact_format(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let total_nanos = if self.0 < 0 {
            // negative duration
            write!(f, "-")?;
            std::ops::Neg::neg(*self).0 // handle negation without overflow
        } else {
            self.0
        };

        let whole_seconds = total_nanos / Self::NANOS_PER_SEC;
        let nanos = total_nanos - Self::NANOS_PER_SEC * whole_seconds;

        let mut seconds_remaining = whole_seconds;
        let mut did_write = false;

        let days = seconds_remaining / Self::SEC_PER_DAY;
        if days > 0 {
            write!(f, "{days}d")?;
            seconds_remaining -= days * Self::SEC_PER_DAY;
            did_write = true;
        }

        let hours = seconds_remaining / Self::SEC_PER_HOUR;
        if hours > 0 {
            if did_write {
                write!(f, " ")?;
            }
            write!(f, "{hours}h")?;
            seconds_remaining -= hours * Self::SEC_PER_HOUR;
            did_write = true;
        }

        let minutes = seconds_remaining / Self::SEC_PER_MINUTE;
        if minutes > 0 {
            if did_write {
                write!(f, " ")?;
            }
            write!(f, "{minutes}m")?;
            seconds_remaining -= minutes * Self::SEC_PER_MINUTE;
            did_write = true;
        }

        const MAX_MILLISECOND_ACCURACY: bool = true;
        const MAX_MICROSECOND_ACCURACY: bool = true;

        if seconds_remaining > 0 || nanos > 0 || !did_write {
            if did_write {
                write!(f, " ")?;
            }

            if nanos == 0 {
                write!(f, "{seconds_remaining}s")?;
            } else if MAX_MILLISECOND_ACCURACY || nanos % 1_000_000 == 0 {
                write!(f, "{}.{:03}s", seconds_remaining, nanos / 1_000_000)?;
            } else if MAX_MICROSECOND_ACCURACY || nanos % 1_000 == 0 {
                write!(f, "{}.{:06}s", seconds_remaining, nanos / 1_000)?;
            } else {
                write!(f, "{seconds_remaining}.{nanos:09}s")?;
            }
        }

        Ok(())
    }
}

impl std::ops::Neg for Duration {
    type Output = Duration;

    #[inline]
    fn neg(self) -> Duration {
        // Handle negation without overflow:
        if self.0 == std::i64::MIN {
            Duration(std::i64::MAX)
        } else {
            Duration(-self.0)
        }
    }
}

impl std::fmt::Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.exact_format(f)
    }
}
