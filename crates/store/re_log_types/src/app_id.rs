use crate::entry_name::MAX_ENTRY_NAME_BYTES;
use crate::{EntryId, EntryName, RecordingId};

// These hash parameters are part of persisted application IDs and must remain stable for
// compatibility.
const TRUNCATED_HASH_ALGORITHM: fn(&[u8], u64) -> u64 = xxhash_rust::xxh64::xxh64;
const TRUNCATED_HASH_SEED: u64 = 0;
const TRUNCATED_HASH_LENGTH: usize = 4;

/// Error returned when constructing an empty [`ApplicationId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidApplicationIdError;

impl std::fmt::Display for InvalidApplicationIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Application ID must not be empty")
    }
}

impl std::error::Error for InvalidApplicationIdError {}

/// The user-chosen name of the application doing the logging.
///
/// Application IDs are really schema names.
/// Every recording using the same schema (approximately!) could share the same blueprint.
///
/// In the context of a remote recording, this is the dataset [`EntryName`].
///
/// Unsupported characters and dots are normalized to `-`.
/// Normalized and truncated IDs receive a short hash suffix derived from the original ID.
///
/// Note: `ApplicationId` is being phased out in favor of [`EntryName`].
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, re_byte_size::SizeBytes, serde::Serialize,
)]
pub struct ApplicationId(EntryName);

impl ApplicationId {
    /// Create a new application ID.
    ///
    /// Unsupported characters and dots are normalized to `-`.
    /// Normalized and truncated IDs receive a four-character hash of the original ID.
    #[inline]
    pub fn try_new(id: impl Into<String>) -> Result<Self, InvalidApplicationIdError> {
        let id = id.into();
        if id.is_empty() {
            return Err(InvalidApplicationIdError);
        }

        let is_valid_application_id_char = |character| {
            // NOTE: Dots are used for folders, which is likely not what users want.
            character != '.' && EntryName::is_valid_char(character)
        };
        let needs_migration = MAX_ENTRY_NAME_BYTES < id.len()
            || id
                .chars()
                .any(|character| !is_valid_application_id_char(character));
        let id = if needs_migration {
            // Note: we compute the hash on the original input!
            let hash = TRUNCATED_HASH_ALGORITHM(id.as_bytes(), TRUNCATED_HASH_SEED) as u16;

            // Each mapped character is one ASCII byte, so `take` enforces the byte limit.
            let prefix = id
                .chars()
                .take(MAX_ENTRY_NAME_BYTES - TRUNCATED_HASH_LENGTH)
                .map(|character| {
                    if is_valid_application_id_char(character) {
                        character
                    } else {
                        '-'
                    }
                })
                .collect::<String>();
            let migrated = format!("{prefix}{hash:04x}");
            re_log::warn_once!(
                "Application ID requires migration to an entry name; using the migrated ID: \
                 {id:?} -> {migrated:?}"
            );
            migrated
        } else {
            id
        };

        Ok(Self(
            EntryName::new(id).expect("migrated application IDs are valid entry names"),
        ))
    }

    /// Create an application ID from a catalog entry ID.
    #[deprecated(
        note = "TODO(RR-1358), TODO(RR-4857): Remove synthetic application IDs from catalog-backed data."
    )]
    #[inline]
    pub fn from_entry_id(entry_id: EntryId) -> Self {
        Self::try_new(entry_id.to_string()).expect("entry IDs are valid application IDs")
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
            std::sync::LazyLock::new(|| ApplicationId::from_static_str("unknown_app_id"));

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
        Self::try_new(format!("app_{}", uuid::Uuid::new_v4().simple()))
            .expect("generated application IDs are valid entry names")
    }

    pub(crate) fn as_recording_id(&self) -> RecordingId {
        RecordingId::from(self.as_str())
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

/// This conversion is intentionally one-way because catalog entries do not necessarily identify
/// applications and `ApplicationId` is being phased out.
impl From<ApplicationId> for EntryName {
    #[inline]
    fn from(application_id: ApplicationId) -> Self {
        application_id.0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_invalid_characters() {
        let application_id =
            ApplicationId::try_new("valid/path\twith💣invalid#characters").unwrap();
        assert!(
            application_id
                .as_str()
                .starts_with("valid-path-with-invalid-characters")
        );
    }

    #[test]
    fn normalizes_dots() {
        let application_id = ApplicationId::try_new("app_01-v2.0").unwrap();
        assert!(application_id.as_str().starts_with("app_01-v2-0"));

        let long_id = format!("app.name{}", "a".repeat(MAX_ENTRY_NAME_BYTES));
        let application_id = ApplicationId::try_new(long_id).unwrap();
        assert!(application_id.as_str().starts_with("app-name"));
    }

    #[test]
    fn preserves_valid_characters() {
        let application_id = ApplicationId::try_new("app_01-v2 [test]:main").unwrap();
        assert_eq!(application_id.as_str(), "app_01-v2 [test]:main");
    }

    #[test]
    fn only_rejects_empty_ids() {
        assert_eq!(
            ApplicationId::try_new("").unwrap_err().to_string(),
            "Application ID must not be empty"
        );
        assert!(ApplicationId::try_new("a".repeat(MAX_ENTRY_NAME_BYTES)).is_ok());
        assert!(ApplicationId::try_new("a".repeat(MAX_ENTRY_NAME_BYTES + 1)).is_ok());
    }

    #[test]
    fn hashes_migrated_ids() {
        let prefix = "a".repeat(MAX_ENTRY_NAME_BYTES - TRUNCATED_HASH_LENGTH);
        let first = ApplicationId::try_new(format!("{prefix}first")).unwrap();
        let second = ApplicationId::try_new(format!("{prefix}second")).unwrap();

        assert_eq!(first.as_str().len(), MAX_ENTRY_NAME_BYTES);
        assert_ne!(first, second);

        let normalized = ApplicationId::try_new(".").unwrap();
        let already_valid = ApplicationId::try_new("-").unwrap();
        assert_eq!(normalized.as_str().len(), 1 + TRUNCATED_HASH_LENGTH);
        assert!(normalized.as_str().starts_with('-'));
        assert_ne!(normalized, already_valid);
    }

    #[test]
    fn migrated_ids_are_ascii_and_limited_in_bytes() {
        let application_id = ApplicationId::try_new("💣".repeat(MAX_ENTRY_NAME_BYTES)).unwrap();
        assert!(application_id.as_str().is_ascii());
        assert_eq!(application_id.as_str().len(), MAX_ENTRY_NAME_BYTES);
    }

    #[test]
    fn converts_entry_id_to_application_id() {
        let entry_id = EntryId::new();
        #[expect(deprecated)]
        let application_id = ApplicationId::from_entry_id(entry_id);
        assert_eq!(application_id.as_str(), entry_id.to_string());
    }

    #[test]
    fn converts_to_entry_name() {
        let application_id = ApplicationId::try_new("my/dataset").unwrap();
        let entry_name = EntryName::from(application_id);
        assert!(entry_name.as_str().starts_with("my-dataset"));
    }
}
