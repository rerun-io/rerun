use super::SpaceViewClass;

impl From<&str> for SpaceViewClass {
    #[inline]
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}
