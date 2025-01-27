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
    fn get_opt(&self, key: &str) -> Option<&str>;

    fn get_or_err(&self, key: &str) -> Result<&str, Self::Error> {
        self.get_opt(key).ok_or_else(|| self.missing_key_error(key))
    }

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

    fn get_opt(&self, key: &str) -> Option<&str> {
        self.get(key).map(|value| value.as_str())
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

    fn get_opt(&self, key: &str) -> Option<&str> {
        self.metadata().get(key).map(|v| v.as_str())
    }
}
