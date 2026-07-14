use std::sync::Arc;

/// Maximum length of an entry name.
const MAX_ENTRY_NAME_LENGTH: usize = 180;

#[derive(Debug)]
pub struct InvalidEntryNameError(String);

impl std::fmt::Display for InvalidEntryNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for InvalidEntryNameError {}

impl From<String> for InvalidEntryNameError {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// A validated entry name.
///
/// Entry names must:
/// - Not be empty
/// - Be at most 180 characters long
/// - Only contain ASCII alphanumeric characters, underscores, hyphens, dots, spaces,
///   brackets, and colons
///
/// Uses an `Arc<str>` internally to allow for cheap cloning.
// TODO(RR-3718): Entry names should support a broader set of characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)] // Only used for tests
#[serde(transparent)]
pub struct EntryName(Arc<str>);

impl EntryName {
    /// The name of the system entries table (`__entries`).
    pub fn entries_table() -> Self {
        #[expect(clippy::unwrap_used)] // Constant string, cannot fail validation.
        Self::new("__entries").unwrap()
    }

    /// Create a new entry name, validating that it conforms to the naming rules.
    pub fn new(name: impl Into<String>) -> Result<Self, InvalidEntryNameError> {
        let name = name.into();

        if name.is_empty() {
            return Err(InvalidEntryNameError("name must not be empty".to_owned()));
        }

        if MAX_ENTRY_NAME_LENGTH < name.len() {
            return Err(InvalidEntryNameError(format!(
                "name '{name}' exceeds maximum length of {MAX_ENTRY_NAME_LENGTH} characters (got {})",
                name.len()
            )));
        }

        if let Some(ch) = name.chars().find(|c| {
            !c.is_ascii_alphanumeric()
                && *c != '_'
                && *c != '-'
                && *c != '.'
                && *c != ' '
                && *c != '['
                && *c != ']'
                && *c != ':'
        }) {
            return Err(InvalidEntryNameError(format!(
                "name '{name}' contains invalid character '{ch}'"
            )));
        }

        Ok(Self(Arc::from(name)))
    }

    /// The name of the blueprint dataset associated with a given dataset entry.
    pub fn blueprint_for(dataset_id: crate::EntryId) -> Self {
        Self::new(format!("__bp_{dataset_id}"))
            .expect("EntryId can always be converted to a valid entry name")
    }

    /// The name of the asset dataset associated with a given dataset entry.
    pub fn asset_for(dataset_id: crate::EntryId) -> Self {
        Self::new(format!("__as_{dataset_id}"))
            .expect("EntryId can always be converted to a valid entry name")
    }

    /// Hidden entries have names starting with `__` (e.g. `__entries`, `__bp_…`).
    pub fn is_hidden(&self) -> bool {
        self.0.starts_with("__")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EntryName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::EntryName;

    #[test]
    fn rejects_empty() {
        assert!(EntryName::new("").is_err());
    }

    #[test]
    fn rejects_too_long() {
        assert!(EntryName::new("a".repeat(181)).is_err());
        assert!(EntryName::new("a".repeat(180)).is_ok());
    }

    #[test]
    fn rejects_invalid_characters() {
        assert!(EntryName::new("no/slashes").is_err());
        assert!(EntryName::new("no\ttabs").is_err());
    }

    #[test]
    fn accepts_valid_names() {
        for name in [
            "a",
            "__entries",
            "my-dataset.v2",
            "with spaces",
            "[bracket]:colon",
        ] {
            assert!(EntryName::new(name).is_ok(), "should accept {name:?}");
        }
    }
}
