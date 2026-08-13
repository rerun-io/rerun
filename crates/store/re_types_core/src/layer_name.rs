/// The name of a layer (e.g. `"base"`).
///
/// Layers partition a segment's chunks into named groups that can be
/// registered, queried, and deleted independently.
//
// NOTE: Intentionally does not implement `Default` — a blank layer name is
// almost always a bug. It cannot be constructed empty at all: use the fallible
// [`LayerName::try_new`], or [`LayerName::base`] when you really want `"base"`.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, ::serde::Serialize)]
pub struct LayerName(String);

/// Error returned when constructing an invalid [`LayerName`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidLayerNameError {
    /// Why the string was rejected, e.g. `"must not be empty"`.
    reason: &'static str,
}

impl std::fmt::Display for InvalidLayerNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid `LayerName`: {}", self.reason)
    }
}

impl std::fmt::Debug for InvalidLayerNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InvalidLayerNameError({:?})", self.reason)
    }
}

impl std::error::Error for InvalidLayerNameError {}

impl LayerName {
    /// The default layer name (`"base"`) used when no explicit layer is specified.
    pub const DEFAULT_STR: &'static str = "base";

    /// Create a new layer name, failing if the string is invalid (e.g. empty).
    #[inline]
    pub fn try_new(name: impl Into<String>) -> Result<Self, InvalidLayerNameError> {
        let name = name.into();

        if name.is_empty() {
            return Err(InvalidLayerNameError {
                reason: "must not be empty",
            });
        }

        Ok(Self(name))
    }

    /// Create from a trusted compile-time string literal.
    ///
    /// # Panics
    /// Panics if `string` is invalid (e.g. empty).
    #[inline]
    pub fn from_static_str(string: &'static str) -> Self {
        Self::try_new(string).unwrap_or_else(|err| panic!("{err} (got {string:?})"))
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

// NOTE: no `TryFrom<&str>` / `TryFrom<&String>`: those would collide with the blanket
// `impl<U: Into<T>> TryFrom<U> for T` in `core` once we implement `From<&'static str>`
// below. Use the inherent `try_new` for fallible construction from borrowed strings.
impl TryFrom<String> for LayerName {
    type Error = InvalidLayerNameError;

    #[inline]
    fn try_from(name: String) -> Result<Self, Self::Error> {
        Self::try_new(name)
    }
}

// Only `&'static str` (string literals / consts), so `impl Into<LayerName>` parameters stay
// ergonomic for trusted compile-time values. Dynamic `&str`/`String` must go through
// the fallible `try_new`/`TryFrom` instead.
impl From<&'static str> for LayerName {
    /// # Panics
    /// Panics if `string` is empty.
    #[inline]
    fn from(string: &'static str) -> Self {
        Self::from_static_str(string)
    }
}

impl From<LayerName> for String {
    #[inline]
    fn from(name: LayerName) -> Self {
        name.0
    }
}

// Fallible, so an empty string is rejected here too (used by e.g. `clap` value parsing).
impl std::str::FromStr for LayerName {
    type Err = InvalidLayerNameError;

    #[inline]
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::try_new(name)
    }
}

// Make `quiver::Column<LayerName>` work (backed by a `Utf8` column).
// `try_*` because reading validates non-emptiness (via `TryFrom<String>`) at
// column construction, so an empty layer name can't sneak in from storage either.
quiver::try_newtype_datatype!(LayerName, quiver::Utf8);

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

impl<'de> serde::Deserialize<'de> for LayerName {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let string = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::try_new(string).map_err(D::Error::custom)
    }
}

impl re_byte_size::SizeBytes for LayerName {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.0.heap_size_bytes()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use super::*;

    #[test]
    fn empty_is_rejected_everywhere() {
        assert!(LayerName::try_new("").is_err());
        assert!(LayerName::try_new(String::new()).is_err());
        assert!(LayerName::from_str("").is_err());
        assert!(LayerName::try_from(String::new()).is_err());
    }

    #[test]
    fn non_empty_round_trips() {
        assert_eq!(LayerName::try_new("base").unwrap().as_str(), "base");
        assert_eq!(LayerName::from_static_str("base").as_str(), "base");
        assert_eq!(LayerName::from("base").as_str(), "base"); // `From<&'static str>`
        assert_eq!("base".parse::<LayerName>().unwrap().as_str(), "base");
        assert_eq!(LayerName::base().as_str(), LayerName::DEFAULT_STR);
    }

    #[test]
    #[should_panic(expected = "must not be empty")]
    fn from_static_str_panics_on_empty() {
        let _ = LayerName::from_static_str("");
    }

    #[test]
    fn serde_rejects_empty() {
        let json = serde_json::to_string(&LayerName::base()).unwrap();
        assert_eq!(json, "\"base\"");
        assert_eq!(
            serde_json::from_str::<LayerName>(&json).unwrap(),
            LayerName::base()
        );
        assert!(serde_json::from_str::<LayerName>("\"\"").is_err());
    }

    #[test]
    fn quiver_column_rejects_empty() {
        use arrow::array::StringArray;

        // A non-empty column round-trips.
        let column = quiver::Column::<LayerName>::from_values([LayerName::base()]);
        assert_eq!(column.to_vec(), [LayerName::base()]);

        // A column containing an empty string is rejected at construction.
        let array = std::sync::Arc::new(StringArray::from(vec!["base", ""]));
        assert!(quiver::Column::<LayerName>::try_new(array).is_err());
    }
}
