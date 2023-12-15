use super::SpaceViewClass;

// TODO(#4536): These should come for free
impl From<&str> for SpaceViewClass {
    #[inline]
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<String> for SpaceViewClass {
    #[inline]
    fn from(value: String) -> Self {
        Self(value.into())
    }
}
