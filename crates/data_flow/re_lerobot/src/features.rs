use std::fmt;

use arrow::array::{Array, StringArray};
use arrow::datatypes::DataType;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, SeqAccess, Visitor},
};

/// Feature definition for a `LeRobot` dataset.
///
/// Each feature represents a data stream recorded during an episode, of a specific data type (`dtype`)
/// and dimensionality (`shape`).
///
/// For example, a shape of `[3, 224, 224]` for a [`DType::Image`] feature denotes a 3-channel
/// (e.g. RGB) image with a height and width of 224 pixels each. The channel axis is first
/// here and last in a `[224, 224, 3]` feature, so [`Self::num_channels`] reads `names` when
/// it can rather than assuming an order.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Feature {
    pub dtype: DType,
    pub shape: Vec<usize>,
    pub names: Option<Names>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info: Option<FeatureInfo>,
}

/// The image channel counts the reader supports: 1 emits a depth image, 3 a color image.
pub const SUPPORTED_CHANNEL_COUNTS: [usize; 2] = [1, 3];

impl Feature {
    /// The channel count of an image feature: the shape entry named `"channel"` or
    /// `"channels"` when the feature's `names` provide one, otherwise whichever end of
    /// `shape` holds a supported channel count. `None` when the shape is empty.
    ///
    /// `LeRobot` writes both `[C, H, W]` and `[H, W, C]`, so neither end can be assumed.
    pub fn num_channels(&self) -> Option<usize> {
        if let Some(names) = &self.names
            && let Some(channel_idx) = names
                .0
                .iter()
                .position(|name| name == "channel" || name == "channels")
            && channel_idx < self.shape.len()
        {
            return Some(self.shape[channel_idx]);
        }
        match (self.shape.first().copied(), self.shape.last().copied()) {
            (Some(first), _) if SUPPORTED_CHANNEL_COUNTS.contains(&first) => Some(first),
            (_, last) => last,
        }
    }
}

/// Encoding details of a video/image feature, from the feature's `info` block.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FeatureInfo {
    /// The camera's own frame rate; falls back to the dataset `fps` when absent.
    #[serde(rename = "video.fps", default, skip_serializing_if = "Option::is_none")]
    pub video_fps: Option<f64>,
}

/// Data types supported for features in a `LeRobot` dataset.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DType {
    Video,
    Image,
    Bool,
    Float32,
    Float64,
    Int16,
    Int64,
    String,
    Language,

    /// A dtype this crate does not know; the feature carrying it emits nothing.
    Unknown,
}

impl<'de> Deserialize<'de> for DType {
    /// An unknown dtype must not abort the whole dataset: it maps to [`Self::Unknown`]
    /// (with a warning naming it), so only that feature is dropped.
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let dtype = String::deserialize(deserializer)?;
        Ok(match dtype.as_str() {
            "video" => Self::Video,
            "image" => Self::Image,
            "bool" => Self::Bool,
            "float32" => Self::Float32,
            "float64" => Self::Float64,
            "int16" => Self::Int16,
            "int64" => Self::Int64,
            "string" => Self::String,
            "language" => Self::Language,
            unknown => {
                re_log::warn_once!(
                    "Unknown LeRobot feature dtype `{unknown}`; features of this dtype are skipped"
                );
                Self::Unknown
            }
        })
    }
}

/// Normalize Arrow string encodings to `Utf8`, rejecting non-string input.
pub fn normalize_string_array(array: &dyn Array) -> Result<StringArray, arrow::error::ArrowError> {
    if !array.data_type().is_string() {
        return Err(arrow::error::ArrowError::CastError(format!(
            "Expected a string array, got {}",
            array.data_type(),
        )));
    }

    let strings = arrow::compute::cast(array, &DataType::Utf8)?;
    Ok(arrow::array::as_string_array(strings.as_ref()).clone())
}

/// Name metadata for a feature in the `LeRobot` dataset.
///
/// The name metadata can consist of
/// - A single string (e.g., `"img_state_delta"`).
/// - A flat list of names for each dimension of a feature (e.g., `["height", "width", "channel"]`).
/// - A nested list of names for each dimension of a feature (e.g., `[["kLeftShoulderPitch", "kLeftShoulderRoll"]]`)
/// - A map with a string array value (e.g., `{ "motors": ["motor_0", "motor_1", …] }` or `{ "axes": ["x", "y", "z"] }`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Names(pub Vec<String>);

/// Visitor implementation for deserializing the [`Names`] type.
///
/// Handles multiple representation formats:
/// - Single strings: `"img_state_delta"`
/// - Flat string arrays: `["x", "y", "z"]`
/// - Nested string arrays: `[["motor_1", "motor_2"]]`
/// - Single-entry objects: `{"motors": ["motor_1", "motor_2"]}` or `{"axes": null}`
///
/// See the `Names` type documentation for more details on the supported formats.
struct NamesVisitor;

impl<'de> Visitor<'de> for NamesVisitor {
    type Value = Names;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "a string, a flat string array, a nested string array, or a single-entry object with a string array or null value",
        )
    }

    /// Handle a single string: `"img_state_delta"`
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Names(vec![v.to_owned()]))
    }

    /// Handle sequences:
    /// - Flat string arrays: `["x", "y", "z"]`
    /// - Nested string arrays: `[["motor_1", "motor_2"]]`
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        // Helper enum to deserialize sequence elements
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum ListItem {
            Str(String),
            List(Vec<String>),
        }

        /// Enum to track the list type
        #[derive(PartialEq)]
        enum ListType {
            Undetermined,
            Flat,
            Nested,
        }

        let mut names = Vec::new();
        let mut determined_type = ListType::Undetermined;

        while let Some(item) = seq.next_element::<ListItem>()? {
            match item {
                ListItem::Str(s) => {
                    if determined_type == ListType::Nested {
                        return Err(serde::de::Error::custom(
                            "Cannot mix nested lists with flat strings within names array",
                        ));
                    }
                    determined_type = ListType::Flat;
                    names.push(s);
                }
                ListItem::List(list) => {
                    if determined_type == ListType::Flat {
                        return Err(serde::de::Error::custom(
                            "Cannot mix flat strings and nested lists within names array",
                        ));
                    }
                    determined_type = ListType::Nested;

                    // Flatten the nested list
                    names.extend(list);
                }
            }
        }

        Ok(Names(names))
    }

    /// Handle single-entry objects: `{"motors": ["motor_1", "motor_2"]}` or `{"axes": null}`
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut names_vec: Option<Vec<String>> = None;
        let mut entry_count = 0;

        // We expect exactly one entry.
        while let Some((_key, value)) = map.next_entry::<String, Option<Vec<String>>>()? {
            entry_count += 1;
            if entry_count > 1 {
                // Consume remaining entries to be a good citizen before erroring
                while map
                    .next_entry::<serde::de::IgnoredAny, serde::de::IgnoredAny>()?
                    .is_some()
                {}

                return Err(serde::de::Error::invalid_length(
                    entry_count,
                    &"a Names object with exactly one entry.",
                ));
            }

            names_vec = Some(value.unwrap_or_default());
        }

        Ok(Names(names_vec.unwrap_or_default()))
    }
}

impl<'de> Deserialize<'de> for Names {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(NamesVisitor)
    }
}

/// A feature's name in the dataset metadata: the key of `info.json`'s `features` map
/// (e.g. `observation.state`).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct FeatureKey(String);

impl FeatureKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FeatureKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Lets a `BTreeMap<FeatureKey, _>` be probed with a plain `&str`, without allocating.
impl std::borrow::Borrow<str> for FeatureKey {
    #[inline]
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<String> for FeatureKey {
    fn from(key: String) -> Self {
        Self(key)
    }
}

impl From<&str> for FeatureKey {
    fn from(key: &str) -> Self {
        Self(key.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_single_string() {
        let json = r#""some_name""#;
        let expected = Names(vec!["some_name".to_owned()]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_flat_list() {
        let json = r#"["a", "b", "c"]"#;
        let expected = Names(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_nested_list() {
        let json = r#"[["a", "b"], ["c"]]"#;
        let expected = Names(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_empty_nested_list() {
        let json = r#"[[], []]"#;
        let expected = Names(vec![]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_empty_list() {
        let json = r#"[]"#;
        let expected = Names(vec![]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_object_with_list() {
        let json = r#"{ "axes": ["x", "y", "z"] }"#;
        let expected = Names(vec!["x".to_owned(), "y".to_owned(), "z".to_owned()]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_object_with_empty_list() {
        let json = r#"{ "motors": [] }"#;
        let expected = Names(vec![]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_object_with_null() {
        let json = r#"{ "axes": null }"#;
        let expected = Names(vec![]); // Null results in an empty list
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_empty_object() {
        // Empty object results in empty list.
        let json = r#"{}"#;
        let expected = Names(vec![]);
        let names: Names = serde_json::from_str(json).unwrap();
        assert_eq!(names, expected);
    }

    #[test]
    fn test_deserialize_error_mixed_list() {
        let json = r#"["a", ["b"]]"#; // Mixed flat and nested
        let result: Result<Names, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Cannot mix flat strings and nested lists")
        );
    }

    #[test]
    fn test_deserialize_feature_with_language_dtype() {
        // Regression: `"language"` features must deserialize rather than
        // aborting the whole import (previously an unknown-variant error).
        let json = r#"{ "dtype": "language", "shape": [1], "names": null }"#;
        let feature: Feature = serde_json::from_str(json).unwrap();
        assert_eq!(feature.dtype, DType::Language);
    }

    #[test]
    fn test_deserialize_error_object_multiple_entries() {
        let json = r#"{ "axes": ["x"], "motors": ["m"] }"#;
        let result: Result<Names, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("a Names object with exactly one entry")
        );
    }
}
