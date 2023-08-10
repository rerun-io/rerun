use super::KeypointId;

impl From<u16> for KeypointId {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl From<crate::components::KeypointId> for KeypointId {
    #[inline]
    fn from(id: crate::components::KeypointId) -> Self {
        Self(id.0)
    }
}
