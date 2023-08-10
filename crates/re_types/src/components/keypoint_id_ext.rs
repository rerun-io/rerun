use super::KeypointId;

impl KeypointId {
    #[inline]
    pub fn new(value: impl Into<crate::datatypes::KeypointId>) -> Self {
        Self(value.into())
    }
}
impl std::fmt::Display for KeypointId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
