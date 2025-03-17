use super::Timestamp;

impl Timestamp {
    /// The current time.
    #[inline]
    pub fn now() -> Self {
        Self(re_log_types::Timestamp::now().ns_since_epoch().into())
    }
}
