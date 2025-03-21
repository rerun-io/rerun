use super::VideoTimestamp;

impl VideoTimestamp {
    /// Create new timestamp from nanoseconds since video start.
    #[inline]
    pub fn from_nanos(nanos: i64) -> Self {
        Self(nanos)
    }

    /// Returns the timestamp as nanoseconds.
    #[inline]
    pub fn as_nanos(self) -> i64 {
        self.0
    }

    /// Returns the timestamp as seconds.
    #[inline]
    pub fn as_secs(self) -> f64 {
        self.0 as f64 / 1e9
    }
}
