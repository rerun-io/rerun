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

impl std::fmt::Display for Duration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.exact_format(f)
    }
}
