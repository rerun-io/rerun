use super::Body;

impl Body {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for Body {
    #[inline]
    fn from(value: String) -> Self {
        Self(value.into())
    }
}

impl From<&str> for Body {
    #[inline]
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}
