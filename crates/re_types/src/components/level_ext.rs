use super::Level;

impl Level {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for Level {
    #[inline]
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Level {
    #[inline]
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}
