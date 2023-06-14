use super::ClassId;

impl From<u16> for ClassId {
    fn from(value: u16) -> Self {
        Self(value)
    }
}
