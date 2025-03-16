use anyhow::Result;

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
    pub fn from_seconds_since_epoch(secs: f64) -> Self {
        Self::from_ns_since_epoch((secs * 1e9).round() as _)
    }

    /// If true, this time is likely relative to unix epoch.
    pub fn is_timestamp(&self) -> bool {
        let nanos_since_epoch = self.nanos_since_epoch();
        let years_since_epoch = nanos_since_epoch / 1_000_000_000 / 60 / 60 / 24 / 365;

        20 <= years_since_epoch && years_since_epoch <= 150
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
