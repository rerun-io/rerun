use std::ops::RangeInclusive;

/// A signed duration represented as nanoseconds since unix epoch
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Duration(i64);

impl Duration {
    pub const MAX: Self = Self(i64::MAX);
    const NANOS_PER_SEC: i64 = 1_000_000_000;

    #[inline]
    pub const fn from_nanos(nanos: i64) -> Self {
        Self(nanos)
    }

    #[inline]
    pub const fn from_micros(micros: i64) -> Self {
        Self::from_nanos(1_000 * micros)
    }

    #[inline]
    pub const fn from_millis(millis: i64) -> Self {
        Self::from_nanos(1_000_000 * millis)
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

    /// The format will omit trailing sub-second zeroes as far as `subsecond_decimals` perimts it.
    pub fn format_secs(self, subsecond_decimals: RangeInclusive<usize>) -> String {
        re_format::DurationFormatOptions::default()
            .with_always_sign(true)
            .with_only_seconds(true)
            .with_min_decimals(*subsecond_decimals.start())
            .with_max_decimals(*subsecond_decimals.end())
            .format_nanos(self.as_nanos())
    }

    pub fn exact_format(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            &re_format::DurationFormatOptions::default()
                .with_always_sign(true)
                .with_only_seconds(false)
                .with_min_decimals(0)
                .with_max_decimals(9)
                .format_nanos(self.as_nanos()),
        )
    }

    pub fn url_format(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            &re_format::DurationFormatOptions::default()
                .with_spaces(false)
                .with_always_sign(true)
                .with_only_seconds(false)
                .with_min_decimals(0)
                .with_max_decimals(9)
                .format_nanos(self.as_nanos()),
        )
    }

    /// Useful when showing dates/times on a timeline and you want it compact.
    ///
    /// When a duration is less than a second, we only show the time from the last whole second.
    pub fn format_subsecond_as_relative(self) -> String {
        let ns = self.as_nanos();

        let fractional_nanos = ns % 1_000_000_000;
        let is_whole_second = fractional_nanos == 0;

        if is_whole_second {
            self.to_string()
        } else {
            // We are in the sub-second resolution.
            // Showing the full time (HH:MM:SS.XXX or 3h 2m 6s …) becomes too long,
            // so instead we switch to showing the time as milliseconds since the last whole second:
            let ms = fractional_nanos as f64 * 1e-6;
            if fractional_nanos % 1_000_000 == 0 {
                format!("{ms:+.0} ms")
            } else if fractional_nanos % 100_000 == 0 {
                format!("{ms:+.1} ms")
            } else if fractional_nanos % 10_000 == 0 {
                format!("{ms:+.2} ms")
            } else if fractional_nanos % 1_000 == 0 {
                format!("{ms:+.3} ms")
            } else if fractional_nanos % 100 == 0 {
                format!("{ms:+.4} ms")
            } else if fractional_nanos % 10 == 0 {
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
        let s = &re_format::remove_number_formatting(s);
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
    fn test_formatting_duration() {
        assert_eq!(
            &Duration::from_micros(42_000_000).format_secs(0..=6),
            "+42s"
        );
        assert_eq!(&Duration::from_micros(69_000).format_secs(0..=6), "+0.069s");
        assert_eq!(
            &Duration::from_micros(69_900).format_secs(0..=6),
            "+0.069 900s"
        );
        assert_eq!(
            &Duration::from_micros(42_123_000_000).format_secs(0..=6),
            "+42 123s"
        );
        assert_eq!(
            &Duration::from_nanos(1_234_567_891).format_secs(0..=6),
            "+1.234 568s",
            "Should limit decimals and round"
        );
    }

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
            Duration::from_str("42µs").unwrap(),
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
