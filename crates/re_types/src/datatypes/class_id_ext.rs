use super::ClassId;

impl From<u16> for ClassId {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for ClassId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
