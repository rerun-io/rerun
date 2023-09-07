use super::Text;

impl Text {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<Text> for String {
    #[inline]
    fn from(value: Text) -> Self {
        value.as_str().to_owned()
    }
}

impl AsRef<str> for Text {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Text {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for Text {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}
