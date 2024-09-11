use super::VideoTimestamp;

impl VideoTimestamp {
    /// Create new timestamp from nanoseconds since video start.
    #[inline]
    pub fn new_nanoseconds(nanos: i64) -> Self {
        crate::datatypes::VideoTimestamp::new_nanoseconds(nanos).into()
    }
}
