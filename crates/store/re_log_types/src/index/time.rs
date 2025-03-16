use crate::{Duration, Timestamp, TimestampFormat};

/// Either a [`Timestamp`] or a [`Duration`].
#[derive(Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct Time(i64);

impl Time {
    #[inline]
    pub fn now() -> Self {
        Self::from_ns_since_epoch(Timestamp::now().ns_since_epoch())
    }

    #[inline]
    pub fn nanos_since_epoch(&self) -> i64 {
        self.0
    }

    #[inline]
    pub fn from_ns_since_epoch(ns_since_epoch: i64) -> Self {
        Self(ns_since_epoch)
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
            Timestamp::from_ns_since_epoch(self.0).format(timestamp_format)
        } else {
            // Relative time
            Duration::from_nanos(nanos_since_epoch).format_seconds()
        }
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
