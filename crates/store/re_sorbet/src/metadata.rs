use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;

use arrow::datatypes::Field as ArrowField;

/// Arrow metadata for an arrow record batch.
pub type ArrowBatchMetadata = HashMap<String, String>;

/// Arrow metadata for a column/field.
pub type ArrowFieldMetadata = HashMap<String, String>;

#[derive(thiserror::Error, Debug)]
#[error("Missing metadata {key:?}")]
pub struct MissingMetadataKey {
    pub key: String,
}

#[derive(thiserror::Error, Debug)]
#[error("Field {field_name:?} is missing metadata {metadata_key:?}")]
pub struct MissingFieldMetadata {
    pub field_name: String,
    pub metadata_key: String,
}

/// Make it more ergonomic to work with arrow metadata.
pub trait MetadataExt {
    type Error;

    fn missing_key_error(&self, key: &str) -> Self::Error;
    fn get_opt(&self, key: &str) -> Option<Cow<'_, str>>;

    fn get_or_err(&self, key: &str) -> Result<Cow<'_, str>, Self::Error> {
        self.get_opt(key).ok_or_else(|| self.missing_key_error(key))
    }

    /// If the key exists and is NOT `false`.
    fn get_bool(&self, key: &str) -> bool {
        self.get_opt(key)
            .map(|value| !matches!(value.to_lowercase().as_str(), "false" | "no"))
            .unwrap_or(false)
    }
}

impl MetadataExt for HashMap<String, String> {
    type Error = MissingMetadataKey;

    fn missing_key_error(&self, key: &str) -> Self::Error {
        MissingMetadataKey {
            key: key.to_owned(),
        }
    }

    fn get_opt(&self, key: &str) -> Option<Cow<'_, str>> {
        self.get(key).map(|value| value.as_str().into())
    }
}

impl MetadataExt for ArrowField {
    type Error = MissingFieldMetadata;

    fn missing_key_error(&self, key: &str) -> Self::Error {
        MissingFieldMetadata {
            field_name: self.name().clone(),
            metadata_key: key.to_owned(),
        }
    }

    fn get_opt(&self, key: &str) -> Option<Cow<'_, str>> {
        self.metadata().get(key).map(|v| v.as_str().into())
    }
}

/// Newtype over metadata which drains entries as they are read, so that the remaining, unread
/// entries may be kept around (using [`Self::residual`]).
pub struct DrainMetadata(RefCell<HashMap<String, String>>);

impl DrainMetadata {
    pub fn new(metadata: HashMap<String, String>) -> Self {
        Self(RefCell::new(metadata))
    }

    pub fn residual(self) -> HashMap<String, String> {
        self.0.into_inner()
    }
}

impl MetadataExt for DrainMetadata {
    type Error = MissingMetadataKey;

    fn missing_key_error(&self, key: &str) -> Self::Error {
        MissingMetadataKey {
            key: key.to_owned(),
        }
    }

    fn get_opt(&self, key: &str) -> Option<Cow<'_, str>> {
        self.0.borrow_mut().remove(key).map(Cow::Owned)
    }
}
