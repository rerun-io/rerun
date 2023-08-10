use super::ClassId;

impl From<u16> for ClassId {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl From<crate::components::ClassId> for ClassId {
    #[inline]
    fn from(id: crate::components::ClassId) -> Self {
        Self(id.0)
    }
}
