use super::EntityPath;

impl EntityPath {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<String> for EntityPath {
    #[inline]
    fn from(value: String) -> Self {
        Self(value.into())
    }
}

impl From<&str> for EntityPath {
    #[inline]
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<EntityPath> for String {
    #[inline]
    fn from(value: EntityPath) -> Self {
        value.as_str().to_owned()
    }
}

impl AsRef<str> for EntityPath {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for EntityPath {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for EntityPath {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}
