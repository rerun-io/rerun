//! String transforms that operate on flat `ArrayRef` values.

use std::sync::Arc;

use arrow::array::{ArrayRef, StringArray};
use re_lenses_core::combinators::{
    Error, StringPrefix, StringSuffix, Transform as _, try_downcast,
};

/// Prepends a prefix to each string value.
pub fn string_prefix(
    prefix: impl Into<String>,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    let transform = StringPrefix::new(prefix);
    move |source: &ArrayRef| {
        let string_array = try_downcast::<StringArray>(source, "string_prefix input")?;
        Ok(transform
            .transform(string_array)?
            .map(|arr| Arc::new(arr) as ArrayRef))
    }
}

/// Prepends a prefix to each non-empty string value.
pub fn string_prefix_nonempty(
    prefix: impl Into<String>,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    let transform = StringPrefix::new(prefix).with_prefix_empty_string(false);
    move |source: &ArrayRef| {
        let string_array = try_downcast::<StringArray>(source, "string_prefix_nonempty input")?;
        Ok(transform
            .transform(string_array)?
            .map(|arr| Arc::new(arr) as ArrayRef))
    }
}

/// Appends a suffix to each string value.
pub fn string_suffix(
    suffix: impl Into<String>,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    let transform = StringSuffix::new(suffix);
    move |source: &ArrayRef| {
        let string_array = try_downcast::<StringArray>(source, "string_suffix input")?;
        Ok(transform
            .transform(string_array)?
            .map(|arr| Arc::new(arr) as ArrayRef))
    }
}

/// Appends a suffix to each non-empty string value.
pub fn string_suffix_nonempty(
    suffix: impl Into<String>,
) -> impl Fn(&ArrayRef) -> Result<Option<ArrayRef>, Error> + Send + Sync {
    let transform = StringSuffix::new(suffix).with_suffix_empty_string(false);
    move |source: &ArrayRef| {
        let string_array = try_downcast::<StringArray>(source, "string_suffix_nonempty input")?;
        Ok(transform
            .transform(string_array)?
            .map(|arr| Arc::new(arr) as ArrayRef))
    }
}
