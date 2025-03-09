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

    /// Try to parse a duration from a string
    pub fn parse(s: &str) -> Option<Self> {
        // Try parsing as a simple relative time with unit suffix (e.g. "1.234s", "1.234ms")
        let suffixes = [("s", 1e9), ("ms", 1e6), ("us", 1e3), ("ns", 1.0)];
        for (suffix, to_ns) in suffixes {
            if let Some(s) = s.strip_suffix(suffix) {
                if let Ok(value) = s.parse::<f64>() {
                    return Some(Self::from_nanos((value * to_ns).round() as i64));
                }
            }
        }

        // Parse a few common ntime formats:
        let time_formats = [
            // Just time with milliseconds
            "[hour]:[minute]:[second].[subsecond]",
            // Just time
            "[hour]:[minute]:[second]",
        ];
        for format in time_formats {
            let format = time::format_description::parse_borrowed::<2>(format)
                .expect("Invalid format string");

            if let Ok(time) = time::Time::parse(s, &format).map(|t| {
                let (h, m, s, ns) = t.as_hms_nano();
                Self::from_nanos(
                    ns as i64
                        + s as i64 * time::convert::Nanosecond::per(time::convert::Second) as i64
                        + m as i64 * time::convert::Nanosecond::per(time::convert::Minute) as i64
                        + h as i64 * time::convert::Nanosecond::per(time::convert::Hour) as i64,
                )
            }) {
                return Some(time);
            }
        }

        None
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

impl From<Duration> for super::TimeInt {
    #[inline]
    fn from(duration: Duration) -> Self {
        Self::saturated_nonstatic_i64(duration.as_nanos())
    }
}

#[cfg(test)]
mod tests {
    use crate::Duration;

    #[test]
    fn parse_duration() {
        assert_eq!(Duration::parse("42s"), Some(Duration::from_secs(42.0)));
        assert_eq!(
            Duration::parse("42.123s"),
            Some(Duration::from_secs(42.123))
        );
        assert_eq!(Duration::parse("42ms"), Some(Duration::from_secs(0.042)));
        assert_eq!(Duration::parse("42us"), Some(Duration::from_secs(0.000042)));
        assert_eq!(
            Duration::parse("42ns"),
            Some(Duration::from_secs(0.000000042))
        );

        // Hour format.
        assert_eq!(
            Duration::parse("22:35:42"),
            Some(Duration::from_secs(22 * 60 * 60 + 35 * 60 + 42))
        );

        // Hout format with fractional seconds.
        assert_eq!(
            Duration::parse("00:00:42.069"),
            Some(Duration::from_nanos(42_069_000_000))
        );

        // Test invalid formats
        assert_eq!(Duration::parse("invalid"), None);
        assert_eq!(Duration::parse("123"), None); // lacks unit
        assert_eq!(Duration::parse("25:00:00"), None); // Invalid hour
        assert_eq!(Duration::parse("00:60:00"), None); // Invalid minute
        assert_eq!(Duration::parse("00:00:60"), None); // Invalid second
    }
}
