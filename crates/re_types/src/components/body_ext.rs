use super::Body;

impl<'s> Body<'s> {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0
    }
}

impl<'s> From<&'s String> for Body<'s> {
    #[inline]
    fn from(value: &'s String) -> Self {
        Self(value.as_str())
    }
}

impl<'s> From<&'s str> for Body<'s> {
    #[inline]
    fn from(value: &'s str) -> Self {
        Self(value)
    }
}
