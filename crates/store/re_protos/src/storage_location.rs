/// A storage-location field does not satisfy its format requirements.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct InvalidStorageStringValue(&'static str);

macro_rules! storage_string {
    ($(#[$meta:meta])* $name:ident, $validate:expr, $error:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// Parses a value without normalizing it.
            ///
            /// Returns an error if the value does not satisfy the type's format requirements.
            pub fn try_new(value: impl Into<String>) -> Result<Self, InvalidStorageStringValue> {
                let value = value.into();
                if !($validate)(&value) {
                    return Err(InvalidStorageStringValue($error));
                }
                Ok(Self(value))
            }

            /// The value as configured by the storage provider.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl TryFrom<String> for $name {
            type Error = InvalidStorageStringValue;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::try_new(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

storage_string!(
    /// A non-empty bucket name, without a URI scheme or object path.
    ///
    /// Provider-specific naming rules are not enforced.
    StorageBucketName,
    |value: &str| !value.is_empty() && !value.contains(['/', ':']),
    "storage bucket name must be non-empty and contain neither '/' nor ':'"
);

storage_string!(
    /// A non-empty object-key prefix without leading or trailing slashes.
    ///
    /// This is an object-store key, not a filesystem path; its bytes are preserved.
    StoragePrefix,
    |value: &str| !value.is_empty() && !value.starts_with('/') && !value.ends_with('/'),
    "storage prefix must be non-empty and have no leading or trailing slash"
);

storage_string!(
    /// A non-empty provider-defined region, including signing regions such as `auto`.
    StorageRegion,
    |value: &str| !value.is_empty(),
    "storage region must not be empty"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_storage_fields_during_deserialization() {
        for bucket in ["", "s3://bucket", "bucket/object"] {
            assert!(
                serde_json::from_value::<StorageBucketName>(serde_json::json!(bucket)).is_err(),
                "{bucket:?}"
            );
        }
        for prefix in ["", "/", "/recordings", "recordings/"] {
            assert!(
                serde_json::from_value::<StoragePrefix>(serde_json::json!(prefix)).is_err(),
                "{prefix:?}"
            );
        }
        assert!(serde_json::from_value::<StorageRegion>(serde_json::json!("")).is_err());
    }

    #[test]
    fn preserves_object_keys() {
        let value = serde_json::json!("recordings/a%2Fb");
        let prefix: StoragePrefix = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(prefix.as_str(), "recordings/a%2Fb");
        assert_eq!(serde_json::to_value(prefix).unwrap(), value);
        assert_eq!(StorageRegion::try_new("auto").unwrap().as_str(), "auto");
    }
}
