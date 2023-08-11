use super::KeypointId;

impl From<u16> for KeypointId {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for KeypointId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
