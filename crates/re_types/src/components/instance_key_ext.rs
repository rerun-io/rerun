use super::InstanceKey;

// TODO(cmc): come up with some DSL in our flatbuffers definitions so that we can declare these
// constants directly in there.
impl InstanceKey {
    /// Draw order used for images if no draw order was specified.
    pub const SPLAT: Self = Self(u64::MAX);
}

impl From<u64> for InstanceKey {
    #[inline]
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<InstanceKey> for u64 {
    #[inline]
    fn from(value: InstanceKey) -> Self {
        value.0
    }
}
