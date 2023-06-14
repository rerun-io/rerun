use super::KeypointId;

impl From<u16> for KeypointId {
    fn from(value: u16) -> Self {
        Self(value)
    }
}
