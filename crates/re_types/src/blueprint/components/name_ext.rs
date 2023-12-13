use super::Name;

impl From<&str> for Name {
    #[inline]
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<String> for Name {
    #[inline]
    fn from(value: String) -> Self {
        Self(value.into())
    }
}
