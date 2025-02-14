use super::Name;

// TODO(#4536): These should come for free
impl Name {
    /// Returns the name as a string slice.
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

impl std::fmt::Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::borrow::Borrow<str> for Name {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Default for Name {
    #[inline]
    fn default() -> Self {
        // Instead of an empty string, put a placeholder there to make it easier to spot
        // the missing name when shown in the viewer.
        Self("<name>".into())
    }
}
