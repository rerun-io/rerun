use super::ClassId;

impl ClassId {
    #[inline]
    pub fn new(value: impl Into<crate::datatypes::ClassId>) -> Self {
        Self(value.into())
    }
}

impl std::fmt::Display for ClassId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
