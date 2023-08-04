use super::Label;

impl<'s> Label<'s> {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0
    }
}

impl<'s> From<&'s String> for Label<'s> {
    #[inline]
    fn from(value: &'s String) -> Self {
        Self(value.as_str())
    }
}

impl<'s> From<&'s str> for Label<'s> {
    #[inline]
    fn from(value: &'s str) -> Self {
        Self(value)
    }
}

impl<'s> From<Label<'s>> for String {
    #[inline]
    fn from(value: Label<'s>) -> Self {
        value.0.to_owned()
    }
}

impl<'s> AsRef<str> for Label<'s> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<'s> std::borrow::Borrow<str> for Label<'s> {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl<'s> std::ops::Deref for Label<'s> {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}
