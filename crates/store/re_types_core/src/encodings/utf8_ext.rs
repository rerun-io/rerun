use crate::ComponentIdentifier;

use super::Utf8;

impl Utf8 {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for Utf8 {
    #[inline]
    fn from(value: String) -> Self {
        Self(value.into())
    }
}

impl From<&str> for Utf8 {
    #[inline]
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<Utf8> for String {
    #[inline]
    fn from(value: Utf8) -> Self {
        value.as_str().to_owned()
    }
}

impl AsRef<str> for Utf8 {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Utf8 {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for Utf8 {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl From<ComponentIdentifier> for Utf8 {
    #[inline]
    fn from(value: ComponentIdentifier) -> Self {
        Self(value.as_str().into())
    }
}

impl std::fmt::Debug for Utf8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}
