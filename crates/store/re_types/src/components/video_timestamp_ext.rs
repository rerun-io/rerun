use super::VideoTimestamp;

impl VideoTimestamp {
    /// Create new timestamp from seconds since video start.
    #[inline]
    pub fn from_secs(seconds: f64) -> Self {
        crate::datatypes::VideoTimestamp::from_nanos((seconds * 1e9).round() as i64).into()
    }

    /// Create new timestamp from seconds since video start.
    #[deprecated(since = "0.23.0", note = "Use `from_secs` instead")]
    #[inline]
    pub fn from_seconds(seconds: f64) -> Self {
        crate::datatypes::VideoTimestamp::from_nanos((seconds * 1e9).round() as i64).into()
    }

    /// Create new timestamp from milliseconds since video start.
    #[inline]
    pub fn from_millis(milliseconds: f64) -> Self {
        crate::datatypes::VideoTimestamp::from_nanos((milliseconds * 1e6).round() as i64).into()
    }

    /// Create new timestamp from milliseconds since video start.
    #[deprecated(since = "0.23.0", note = "Use `from_millis` instead")]
    #[inline]
    pub fn from_milliseconds(milliseconds: f64) -> Self {
        crate::datatypes::VideoTimestamp::from_nanos((milliseconds * 1e6).round() as i64).into()
    }

    /// Create new timestamp from nanoseconds since video start.
    #[inline]
    pub fn from_nanos(nanos: i64) -> Self {
        crate::datatypes::VideoTimestamp::from_nanos(nanos).into()
    }

    /// Create new timestamp from nanoseconds since video start.
    #[deprecated(since = "0.23.0", note = "Use `from_nanos` instead")]
    #[inline]
    pub fn from_nanoseconds(nanos: i64) -> Self {
        crate::datatypes::VideoTimestamp::from_nanos(nanos).into()
    }
}
