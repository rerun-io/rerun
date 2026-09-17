//! Shared logic for the `asset` property consumed by `GetAssetsForSegment`.
//!
//! The `asset` property holds two components: [`ASSET_MODE_COMPONENT`], one of `OptIn`/`OptOut`,
//! and [`ASSET_SEGMENTS_COMPONENT`], the list of segment ids the mode applies to. An `OptOut` asset
//! applies to every segment except those listed. An `OptIn` asset applies only to those listed. An
//! asset with no `asset` property defaults to `OptOut` with an empty list, so it applies to every
//! segment.

use std::collections::HashSet;

use arrow::array::{Array, AsArray as _, RecordBatch};

pub const ASSET_PROPERTY: &str = "asset";
pub const ASSET_MODE_COMPONENT: &str = "mode";
pub const ASSET_SEGMENTS_COMPONENT: &str = "segments";

/// How an asset's segment list is interpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AssetMode {
    /// The asset applies only to the segments in its list.
    OptIn,

    /// The asset applies to every segment except those in its list. The default.
    #[default]
    OptOut,
}

impl AssetMode {
    /// The string stored in the [`ASSET_MODE_COMPONENT`].
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OptIn => "OptIn",
            Self::OptOut => "OptOut",
        }
    }

    /// Parse the [`ASSET_MODE_COMPONENT`] string, defaulting to [`AssetMode::OptOut`] for a missing
    /// or unrecognized value.
    pub fn parse_or_default(mode: Option<&str>) -> Self {
        match mode {
            Some("OptIn") => Self::OptIn,
            _ => Self::OptOut,
        }
    }
}

/// Decide whether an asset applies to `requested_segment`, given the asset's mode and its own list
/// of segment ids.
///
/// A request with no segment id resolves the per-asset defaults: `OptOut` assets apply, `OptIn`
/// assets don't.
pub fn asset_applies_to_segment(
    mode: AssetMode,
    requested_segment: Option<&str>,
    asset_segments: &HashSet<String>,
) -> bool {
    match (mode, requested_segment) {
        (AssetMode::OptIn, Some(segment)) => asset_segments.contains(segment),
        (AssetMode::OptIn, None) => false,
        (AssetMode::OptOut, Some(segment)) => !asset_segments.contains(segment),
        (AssetMode::OptOut, None) => true,
    }
}

/// Build the `asset` property of an asset that applies per `mode` and `segments`.
///
/// The inverse of [`read_asset_mode`] and [`read_asset_segments`]: a single-row batch with one
/// list-wrapped column per component, which is how properties are stored and how the segment table
/// surfaces them.
pub fn asset_properties<'a>(
    mode: AssetMode,
    segments: impl IntoIterator<Item = &'a str>,
) -> RecordBatch {
    let segments: Vec<&str> = segments.into_iter().collect();

    RecordBatch::try_from_iter([
        (
            property_column(ASSET_PROPERTY, ASSET_MODE_COMPONENT),
            string_list(&[mode.as_str()]),
        ),
        (
            property_column(ASSET_PROPERTY, ASSET_SEGMENTS_COMPONENT),
            string_list(&segments),
        ),
    ])
    .expect("a batch of equal-length single-row list columns is always valid")
}

/// A single-row `List<Utf8>` column holding `values`.
fn string_list(values: &[&str]) -> arrow::array::ArrayRef {
    use arrow::array::{ListArray, StringArray};
    use arrow::buffer::OffsetBuffer;
    use arrow::datatypes::{DataType, Field};
    use std::sync::Arc;

    let values = Arc::new(StringArray::from(values.to_vec()));
    let field = Arc::new(Field::new("item", DataType::Utf8, true));
    Arc::new(ListArray::new(
        field,
        OffsetBuffer::from_lengths([values.len()]),
        values,
        None,
    ))
}

/// Read the [`AssetMode`] of an asset from its `asset` property at `row`, defaulting to
/// [`AssetMode::OptOut`] when unset.
pub fn read_asset_mode(batch: &RecordBatch, row: usize) -> AssetMode {
    let column = property_column(ASSET_PROPERTY, ASSET_MODE_COMPONENT);
    let mode = list_values_at(batch, row, &column).and_then(|values| first_string(&values));
    AssetMode::parse_or_default(mode.as_deref())
}

/// Read the segment ids an asset opts in or out from its `asset` property at `row`.
///
/// Returns an empty set if the property is absent or the row is null.
pub fn read_asset_segments(batch: &RecordBatch, row: usize) -> HashSet<String> {
    let column = property_column(ASSET_PROPERTY, ASSET_SEGMENTS_COMPONENT);
    list_values_at(batch, row, &column)
        .map(|values| string_values(&values))
        .unwrap_or_default()
}

/// The column holding component `component` of the property logged under
/// `__properties/{property_key}`, i.e. `property:{property_key}:{component}`.
fn property_column(property_key: &str, component: &str) -> String {
    format!("property:{property_key}:{component}")
}

/// Return the list-element array for `column` at `row`, handling both `List` and `LargeList`.
fn list_values_at(batch: &RecordBatch, row: usize, column: &str) -> Option<arrow::array::ArrayRef> {
    let col = batch.column_by_name(column)?;
    if row >= col.len() {
        return None;
    }
    if let Some(list) = col.as_list_opt::<i32>() {
        return list.is_valid(row).then(|| list.value(row));
    }
    if let Some(list) = col.as_list_opt::<i64>() {
        return list.is_valid(row).then(|| list.value(row));
    }
    None
}

/// Collect the non-null strings of a `Utf8`, `LargeUtf8`, or `Utf8View` array.
fn string_values(values: &dyn Array) -> HashSet<String> {
    let collect = |len: usize, valid: &dyn Fn(usize) -> bool, value: &dyn Fn(usize) -> String| {
        (0..len)
            .filter(|&i| valid(i))
            .map(value)
            .collect::<HashSet<_>>()
    };

    if let Some(array) = values.as_string_opt::<i32>() {
        return collect(array.len(), &|i| array.is_valid(i), &|i| {
            array.value(i).to_owned()
        });
    }
    if let Some(array) = values.as_string_opt::<i64>() {
        return collect(array.len(), &|i| array.is_valid(i), &|i| {
            array.value(i).to_owned()
        });
    }
    if let Some(array) = values.as_string_view_opt() {
        return collect(array.len(), &|i| array.is_valid(i), &|i| {
            array.value(i).to_owned()
        });
    }
    HashSet::new()
}

/// The first non-null string of a `Utf8`, `LargeUtf8`, or `Utf8View` array, in order.
///
/// Used to read a scalar string component such as the asset mode, which is stored list-wrapped like
/// any other property.
fn first_string(values: &dyn Array) -> Option<String> {
    let first = |len: usize, valid: &dyn Fn(usize) -> bool, value: &dyn Fn(usize) -> String| {
        (0..len).find(|&i| valid(i)).map(value)
    };

    if let Some(array) = values.as_string_opt::<i32>() {
        return first(array.len(), &|i| array.is_valid(i), &|i| {
            array.value(i).to_owned()
        });
    }
    if let Some(array) = values.as_string_opt::<i64>() {
        return first(array.len(), &|i| array.is_valid(i), &|i| {
            array.value(i).to_owned()
        });
    }
    if let Some(array) = values.as_string_view_opt() {
        return first(array.len(), &|i| array.is_valid(i), &|i| {
            array.value(i).to_owned()
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::StringArray;

    use super::*;

    #[test]
    fn reads_mode_and_segments_from_asset_property() {
        let batch = asset_properties(AssetMode::OptIn, ["a", "b"]);
        assert_eq!(read_asset_mode(&batch, 0), AssetMode::OptIn);
        assert_eq!(
            read_asset_segments(&batch, 0),
            HashSet::from(["a".to_owned(), "b".to_owned()])
        );
    }

    /// An asset that lists no segments still round-trips its mode, which is what an `OptIn` asset
    /// applying to nothing and an `OptOut` asset applying to everything both look like.
    #[test]
    fn reads_mode_with_an_empty_segment_list() {
        let batch = asset_properties(AssetMode::OptOut, []);
        assert_eq!(read_asset_mode(&batch, 0), AssetMode::OptOut);
        assert!(read_asset_segments(&batch, 0).is_empty());
    }

    #[test]
    fn asset_without_property_defaults_to_opt_out() {
        let batch = RecordBatch::try_from_iter([(
            "property:unrelated:value",
            Arc::new(StringArray::from(vec!["x"])) as _,
        )])
        .unwrap();
        assert_eq!(read_asset_mode(&batch, 0), AssetMode::OptOut);
        assert!(read_asset_segments(&batch, 0).is_empty());
    }

    #[test]
    fn opt_in_asset_applies_only_to_listed_segments() {
        let segments = HashSet::from(["wanted".to_owned()]);

        assert!(asset_applies_to_segment(
            AssetMode::OptIn,
            Some("wanted"),
            &segments
        ));
        assert!(!asset_applies_to_segment(
            AssetMode::OptIn,
            Some("other"),
            &segments
        ));
        assert!(!asset_applies_to_segment(AssetMode::OptIn, None, &segments));
    }

    #[test]
    fn opt_out_asset_applies_unless_listed() {
        let segments = HashSet::from(["unwanted".to_owned()]);

        assert!(asset_applies_to_segment(
            AssetMode::OptOut,
            Some("shared"),
            &segments
        ));
        assert!(!asset_applies_to_segment(
            AssetMode::OptOut,
            Some("unwanted"),
            &segments
        ));
        assert!(asset_applies_to_segment(AssetMode::OptOut, None, &segments));
    }
}
