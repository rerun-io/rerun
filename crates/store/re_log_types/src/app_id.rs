use std::sync::Arc;

use crate::{EntryId, RecordingId};

/// Error returned when constructing an invalid [`ApplicationId`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct InvalidApplicationIdError {
    /// Why the string was rejected, e.g. `"must not be empty"`.
    reason: &'static str,
}

impl std::fmt::Display for InvalidApplicationIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid `ApplicationId`: {}", self.reason)
    }
}

impl std::fmt::Debug for InvalidApplicationIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "InvalidApplicationIdError({:?})", self.reason)
    }
}

impl std::error::Error for InvalidApplicationIdError {}

/// The user-chosen name of the application doing the logging.
///
/// Application IDs are really schema names.
/// Every recording using the same schema (approximately!) could share the same blueprint.
///
/// In the context of a remote recording, this is the dataset entry id.
///
/// Guaranteed to never be empty.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, re_byte_size::SizeBytes, serde::Serialize,
)]
pub struct ApplicationId(Arc<String>);

impl ApplicationId {
    /// Create a new application id, failing if the string is invalid (e.g. empty).
    ///
    /// This is the single place where the naming rules are enforced.
    /// Currently only forbids the empty string, but this is where future rules
    /// (e.g. no whitespace) would go.
    #[inline]
    pub fn try_new(id: impl Into<String>) -> Result<Self, InvalidApplicationIdError> {
        let id = id.into();
        if id.is_empty() {
            return Err(InvalidApplicationIdError {
                reason: "must not be empty",
            });
        }
        Ok(Self(Arc::new(id)))
    }

    /// Create from a trusted compile-time string literal.
    ///
    /// # Panics
    /// Panics if `string` is invalid (e.g. empty).
    #[inline]
    pub fn from_static_str(string: &'static str) -> Self {
        match Self::try_new(string) {
            Ok(slf) => slf,
            Err(err) => panic!("{err} (got {string:?})"),
        }
    }

    /// The default [`ApplicationId`] if the user hasn't set one.
    ///
    /// Currently: `"unknown_app_id"`.
    pub fn unknown() -> Self {
        static UNKNOWN_APP_ID: std::sync::LazyLock<ApplicationId> =
            std::sync::LazyLock::new(|| ApplicationId(Arc::new("unknown_app_id".to_owned())));

        UNKNOWN_APP_ID.clone()
    }

    /// Create a new application id, falling back to [`Self::unknown`] if the string is invalid (e.g. empty).
    #[inline]
    pub fn new_or_unknown(id: impl Into<String>) -> Self {
        Self::try_new(id).unwrap_or_else(|_| Self::unknown())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// A randomly generated app id
    pub fn random() -> Self {
        Self(Arc::new(format!("app_{}", uuid::Uuid::new_v4().simple())))
    }

    pub(crate) fn as_recording_id(&self) -> RecordingId {
        RecordingId(Arc::clone(&self.0))
    }
}

// NOTE: no `TryFrom<&str>` / `TryFrom<&String>`: those would collide with the blanket
// `impl<U: Into<T>> TryFrom<U> for T` in `core` once we implement `From<&'static str>`
// below. Use the inherent `try_new` for fallible construction from borrowed strings.
impl TryFrom<String> for ApplicationId {
    type Error = InvalidApplicationIdError;

    #[inline]
    fn try_from(string: String) -> Result<Self, Self::Error> {
        Self::try_new(string)
    }
}

// Only `&'static str` (string literals / consts), so `impl Into<Self>` parameters stay
// ergonomic for trusted compile-time values. Dynamic `&str`/`String` must go through
// the fallible `try_new`/`TryFrom` instead.
impl From<&'static str> for ApplicationId {
    /// # Panics
    /// Panics if `string` is empty.
    #[inline]
    fn from(string: &'static str) -> Self {
        Self::from_static_str(string)
    }
}

impl From<EntryId> for ApplicationId {
    /// In the context of a remote recording, the application id is the dataset entry id.
    #[inline]
    fn from(entry_id: EntryId) -> Self {
        // A formatted `Tuid` is never empty.
        Self(Arc::new(entry_id.to_string()))
    }
}

impl<'de> serde::Deserialize<'de> for ApplicationId {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let string = String::deserialize(deserializer)?;
        Self::try_new(string).map_err(D::Error::custom)
    }
}

impl std::fmt::Display for ApplicationId {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
