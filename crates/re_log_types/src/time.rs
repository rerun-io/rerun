use std::ops::RangeInclusive;

/// A date-time represented as nanoseconds since unix epoch
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Time(i64);

impl Time {
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub fn now() -> Self {
        let nanos_since_epoch = std::time::SystemTime::UNIX_EPOCH
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
    pub fn is_abolute_date(&self) -> bool {
        let nanos_since_epoch = self.nanos_since_epoch();
        let years_since_epoch = nanos_since_epoch / 1_000_000_000 / 60 / 60 / 24 / 365;

        20 <= years_since_epoch && years_since_epoch <= 150
    }

    /// Human-readable formatting
    pub fn format(&self) -> String {
        let nanos_since_epoch = self.nanos_since_epoch();

        if self.is_abolute_date() {
            use chrono::TimeZone as _;
            let datetime = chrono::Utc.timestamp_opt(
                nanos_since_epoch / 1_000_000_000,
                (nanos_since_epoch % 1_000_000_000) as _,
            );
            match datetime {
                chrono::LocalResult::Single(datetime) => {
                    if datetime.date_naive() == chrono::offset::Utc::now().date_naive() {
                        datetime.format("%H:%M:%S%.6fZ").to_string()
                    } else {
                        datetime.format("%Y-%m-%d %H:%M:%S%.6fZ").to_string()
                    }
                }
                chrono::LocalResult::None => "Invalid timestamp".to_owned(),
                chrono::LocalResult::Ambiguous(_, _) => "Ambiguous timestamp".to_owned(),
            }
        } else {
            let secs = nanos_since_epoch as f64 * 1e-9;
            // assume relative time
            format!("{secs:+.03}s")
        }
    }

    pub fn format_time(&self, format_str: &str) -> String {
        use chrono::TimeZone as _;
        let nanos_since_epoch = self.nanos_since_epoch();
        let datetime = chrono::Utc.timestamp_opt(
            nanos_since_epoch / 1_000_000_000,
            (nanos_since_epoch % 1_000_000_000) as _,
        );
        match datetime {
            chrono::LocalResult::Single(datetime) => datetime.format(format_str).to_string(),
            chrono::LocalResult::None => "Invalid timestamp".to_owned(),
            chrono::LocalResult::Ambiguous(_, _) => "Ambiguous timestamp".to_owned(),
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
