use super::Level;

impl<'s> Level<'s> {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0
    }
}

impl<'s> From<&'s String> for Level<'s> {
    #[inline]
    fn from(value: &'s String) -> Self {
        Self(value.as_str())
    }
}

impl<'s> From<&'s str> for Level<'s> {
    #[inline]
    fn from(value: &'s str) -> Self {
        Self(value)
    }
}
