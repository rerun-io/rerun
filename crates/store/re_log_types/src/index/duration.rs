/// A signed duration represented as nanoseconds since unix epoch
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Duration(i64);

impl Duration {
    pub const MAX: Self = Self(i64::MAX);
    const NANOS_PER_SEC: i64 = 1_000_000_000;
    const NANOS_PER_MILLI: i64 = 1_000_000;
    const SEC_PER_MINUTE: i64 = 60;
    const SEC_PER_HOUR: i64 = 60 * Self::SEC_PER_MINUTE;
    const SEC_PER_DAY: i64 = 24 * Self::SEC_PER_HOUR;

    #[inline]
    pub const fn from_nanos(nanos: i64) -> Self {
        Self(nanos)
    }

    #[inline]
    pub const fn from_millis(millis: i64) -> Self {
        Self(millis * Self::NANOS_PER_MILLI)
    }

    #[inline]
    pub fn from_secs(secs: impl Into<f64>) -> Self {
        let secs = secs.into();
        Self::from_nanos((secs * Self::NANOS_PER_SEC as f64).round() as _)
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

    /// Format as seconds, approximately.
    pub fn format_seconds(self) -> String {
        let nanos = self.as_nanos();
        let secs = nanos as f64 * 1e-9;

        let is_whole_second = nanos % 1_000_000_000 == 0;

        let secs = re_format::FloatFormatOptions::DEFAULT_f64
            .with_always_sign(true)
            .with_decimals(if is_whole_second { 0 } else { 3 })
            .with_strip_trailing_zeros(false)
            .format(secs);
        format!("{secs}s")
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

    /// Useful when showing dates/times on a timeline and you want it compact.
    ///
    /// When a duration is less than a second, we only show the time from the last whole second.
    pub fn format_subsecond_as_relative(self) -> String {
        let ns = self.as_nanos();

        let fractional_ns = ns % 1_000_000_000;
        let is_whole_second = fractional_ns == 0;

        if is_whole_second {
            self.to_string()
        } else {
            // We are in the sub-second resolution.
            // Showing the full time (HH:MM:SS.XXX or 3h 2m 6s …) becomes too long,
            // so instead we switch to showing the time as milliseconds since the last whole second:
            let ms = fractional_ns as f64 * 1e-6;
            if fractional_ns % 1_000_000 == 0 {
                format!("{ms:+.0} ms")
            } else if fractional_ns % 100_000 == 0 {
                format!("{ms:+.1} ms")
            } else if fractional_ns % 10_000 == 0 {
                format!("{ms:+.2} ms")
            } else if fractional_ns % 1_000 == 0 {
                format!("{ms:+.3} ms")
            } else if fractional_ns % 100 == 0 {
                format!("{ms:+.4} ms")
            } else if fractional_ns % 10 == 0 {
                format!("{ms:+.5} ms")
            } else {
                format!("{ms:+.6} ms")
            }
        }
    }
}

impl From<std::time::Duration> for Duration {
    #[inline]
    fn from(duration: std::time::Duration) -> Self {
        Self::from_nanos(duration.as_nanos() as _)
    }
}

impl std::ops::Neg for Duration {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        // Handle negation without overflow:
        if self.0 == i64::MIN {
            Self(i64::MAX)
        } else {
            Self(-self.0)
        }
    }
}

impl From<Duration> for super::TimeInt {
    #[inline]
    fn from(duration: Duration) -> Self {
        Self::saturated_temporal_i64(duration.as_nanos())
    }
}

// ------------------------------------------
// Formatting and parsing

impl std::str::FromStr for Duration {
    type Err = jiff::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let jiff_timestamp = jiff::SignedDuration::from_str(s)?;
        Ok(Self(jiff_timestamp.as_nanos() as i64))
    }
}

impl std::fmt::Debug for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.exact_format(f)
    }
}

impl std::fmt::Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.exact_format(f)
    }
}

// ------------------------------------------

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use crate::Duration;

    #[test]
    fn parse_duration() {
        assert_eq!(
            Duration::from_str("42s").unwrap(),
            Duration::from_secs(42.0)
        );
        assert_eq!(
            Duration::from_str("42.123s").unwrap(),
            Duration::from_secs(42.123)
        );
        assert_eq!(
            Duration::from_str("42ms").unwrap(),
            Duration::from_secs(0.042)
        );
        assert_eq!(
            Duration::from_str("42us").unwrap(),
            Duration::from_secs(0.000042)
        );
        assert_eq!(
            Duration::from_str("42ns").unwrap(),
            Duration::from_secs(0.000000042)
        );

        // Hour format.
        assert_eq!(
            Duration::from_str("22:35:42").unwrap(),
            Duration::from_secs(22 * 60 * 60 + 35 * 60 + 42)
        );

        // Hout format with fractional seconds.
        assert_eq!(
            Duration::from_str("00:00:42.069").unwrap(),
            Duration::from_nanos(42_069_000_000)
        );

        // Test invalid formats
        assert!(Duration::from_str("invalid").is_err());
        assert!(Duration::from_str("123").is_err());
    }
}
