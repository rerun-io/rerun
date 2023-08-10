use super::ClassId;

impl From<u16> for ClassId {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}
