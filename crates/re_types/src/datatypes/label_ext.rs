use super::Label;

impl Label {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<crate::components::Label> for Label {
    #[inline]
    fn from(l: crate::components::Label) -> Self {
        Self(l.0)
    }
}

impl From<String> for Label {
    #[inline]
    fn from(value: String) -> Self {
        Self(value.into())
    }
}

impl From<&str> for Label {
    #[inline]
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<Label> for String {
    #[inline]
    fn from(value: Label) -> Self {
        value.as_str().to_owned()
    }
}

impl AsRef<str> for Label {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Label {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for Label {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}
