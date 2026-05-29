/// The name of a layer (e.g. `"base"`).
///
/// Layers partition a segment's chunks into named groups that can be
/// registered, queried, and deleted independently.
//
// NOTE: Intentionally does not implement `Default` — a blank layer name is
// almost always a bug. Use [`LayerName::base`] when you really want `"base"`.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(::serde::Deserialize, ::serde::Serialize))]
pub struct LayerName(String);

impl LayerName {
    /// The default layer name (`"base"`) used when no explicit layer is specified.
    pub const DEFAULT_STR: &'static str = "base";

    #[inline]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// The default layer (`"base"`).
    #[inline]
    pub fn base() -> Self {
        Self(Self::DEFAULT_STR.to_owned())
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[inline]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<&str> for LayerName {
    #[inline]
    fn from(name: &str) -> Self {
        Self(name.to_owned())
    }
}

impl std::str::FromStr for LayerName {
    type Err = std::convert::Infallible;

    #[inline]
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Ok(Self(name.to_owned()))
    }
}

impl From<String> for LayerName {
    #[inline]
    fn from(name: String) -> Self {
        Self(name)
    }
}

impl From<&String> for LayerName {
    #[inline]
    fn from(name: &String) -> Self {
        Self(name.clone())
    }
}

impl From<LayerName> for String {
    #[inline]
    fn from(name: LayerName) -> Self {
        name.0
    }
}

impl AsRef<str> for LayerName {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for LayerName {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for LayerName {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl PartialEq<str> for LayerName {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for LayerName {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<LayerName> for str {
    #[inline]
    fn eq(&self, other: &LayerName) -> bool {
        self == other.0
    }
}

impl PartialEq<LayerName> for &str {
    #[inline]
    fn eq(&self, other: &LayerName) -> bool {
        *self == other.0
    }
}

impl re_byte_size::SizeBytes for LayerName {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }
}
