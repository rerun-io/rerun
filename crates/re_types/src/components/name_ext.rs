use super::Name;

// TODO(#4536): These should come for free
impl Name {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<Name> for String {
    #[inline]
    fn from(value: Name) -> Self {
        value.as_str().to_owned()
    }
}

impl AsRef<str> for Name {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Name {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}
