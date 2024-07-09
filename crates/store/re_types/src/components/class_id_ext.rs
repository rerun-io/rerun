use super::ClassId;

impl std::fmt::Display for ClassId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Default for ClassId {
    #[inline]
    fn default() -> Self {
        Self(0.into())
    }
}
