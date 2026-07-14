//! Helpers for working with arrow

mod arrays;
mod batches;
mod compare;
mod format;
mod string_view;
#[cfg(feature = "test")]
mod test_extensions;

// ----------------------------------------------------------------
use std::sync::Arc;

use arrow::array::{Array as _, AsArray as _, ListArray};
use arrow::datatypes::{DataType, Field};

pub use self::arrays::*;
pub use self::batches::*;
pub use self::compare::*;
pub use self::format::{
    RecordBatchFormatOpts, format_field_datatype, format_record_batch, format_record_batch_opts,
    format_record_batch_with_width,
};
pub use self::string_view::*;
#[cfg(feature = "test")]
pub use self::test_extensions::*;

/// Convert any `BinaryArray` to `LargeBinaryArray`, because we treat them logically the same
pub fn widen_binary_arrays(list_array: &ListArray) -> ListArray {
    let list_data_type = list_array.data_type();
    if let DataType::List(field) = list_data_type
        && field.data_type() == &DataType::Binary
    {
        re_tracing::profile_function!();
        let large_binary_field = Field::new("item", DataType::LargeBinary, true);
        let target_type = DataType::List(Arc::new(large_binary_field));

        #[expect(clippy::unwrap_used)]
        arrow::compute::kernels::cast::cast(list_array, &target_type)
            .unwrap()
            .as_list()
            .clone()
    } else {
        list_array.clone()
    }
}

#[cfg(test)]
mod tests {
    use arrow::array::{BinaryBuilder, ListBuilder};

    use super::*;

    #[test]
    fn test_widen_list_binary() {
        // Create test data
        let mut list_builder = ListBuilder::new(BinaryBuilder::new());

        // First list: [b"hello", b"world"]
        list_builder.values().append_value(b"hello");
        list_builder.values().append_value(b"world");
        list_builder.append(true);

        // Second list: [b"rust", b"arrow"]
        list_builder.values().append_value(b"rust");
        list_builder.values().append_value(b"arrow");
        list_builder.append(true);

        // Third list: null
        list_builder.append_null();

        let original_list = list_builder.finish();

        // Widen to LargeBinaryArray
        let widened_list = widen_binary_arrays(&original_list);

        // Verify the result
        assert_eq!(widened_list.len(), 3);
        assert!(!widened_list.is_null(0));
        assert!(!widened_list.is_null(1));
        assert!(widened_list.is_null(2));

        // Check data type
        if let DataType::List(field) = widened_list.data_type() {
            assert_eq!(field.data_type(), &DataType::LargeBinary);
        } else {
            panic!("Expected List data type");
        }
    }
}

// ----------------------------------------------------------------

/// Safety gate: reject [`DataType::Union`] in the checked type, and recursively within
/// nested [`DataType::Struct`], [`DataType::List`], [`DataType::LargeList`], and
/// [`DataType::FixedSizeList`] children.
///
/// This guards merges that would let `Field::try_merge` produce a shape the read-side
/// aligner ([`align_record_batch_to_schema`](../../re_dataframe/utils/fn.align_record_batch_to_schema.html))
/// cannot adapt. In particular, `try_merge` has a recursive Union arm that can widen
/// children, but the aligner has no Union branch.
///
/// Known over-rejection: this check inspects only a single datatype tree, not both the
/// current and incoming shapes, so it cannot tell "Union about to widen" (unsafe) from
/// "Union identical across partitions, only a sibling field widens" (would be safe — the
/// aligner's fast-path handles identical Unions). A two-tree check could close this gap; see
/// the `union_over_rejected_when_only_a_sibling_widens` test for a pinned example. In
/// practice this over-rejection only surfaces when a field that *contains* a Union also
/// changes in some unrelated way across partitions.
///
/// Callers use this as a pre-merge guard on the datatypes they are about to merge.
pub fn reject_unsupported_widenings(dt: &DataType) -> Result<(), arrow::error::ArrowError> {
    match dt {
        DataType::Union(_, _) => Err(arrow::error::ArrowError::SchemaError(
            "union-typed fields in the checked datatype are not supported for schema merging"
                .to_owned(),
        )),
        DataType::Struct(fields) => {
            for f in fields {
                reject_unsupported_widenings(f.data_type())?;
            }
            Ok(())
        }
        DataType::List(f) | DataType::LargeList(f) | DataType::FixedSizeList(f, _) => {
            reject_unsupported_widenings(f.data_type())
        }
        _ => Ok(()),
    }
}

// ----------------------------------------------------------------

/// Error used when a column is missing from a record batch
#[derive(Debug, Clone, thiserror::Error)]
pub struct MissingColumnError {
    pub missing: String,
    pub available: Vec<String>,
}

impl std::fmt::Display for MissingColumnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { missing, available } = self;
        write!(f, "Missing column: {missing:?}. Available: {available:?}")
    }
}

// ----------------------------------------------------------------

/// Error used for arrow datatype mismatch.
#[derive(Debug, Clone, thiserror::Error)]
pub struct WrongDatatypeError {
    pub column_name: Option<String>,
    pub expected: Box<DataType>,
    pub actual: Box<DataType>,
}

impl WrongDatatypeError {
    pub fn ensure_datatype(field: &Field, expected: &DataType) -> Result<(), Self> {
        if field.data_type() == expected {
            Ok(())
        } else {
            Err(Self {
                column_name: Some(field.name().to_owned()),
                expected: expected.clone().into(),
                actual: field.data_type().clone().into(),
            })
        }
    }
}

impl std::fmt::Display for WrongDatatypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            column_name,
            expected,
            actual,
        } = self;
        if let Some(column_name) = column_name {
            write!(
                f,
                "Expected column {column_name:?} to be {expected}, got {actual}"
            )
        } else {
            write!(f, "Expected {expected}, got {actual}")
        }
    }
}

#[cfg(test)]
mod reject_unsupported_widenings_tests {
    use super::*;
    use arrow::datatypes::{DataType, Field, Fields};

    fn small_union_type() -> DataType {
        use arrow::datatypes::UnionFields;
        let fields = UnionFields::try_new(vec![0], vec![Field::new("a", DataType::Int32, true)])
            .expect("valid union fields");
        DataType::Union(fields, arrow::datatypes::UnionMode::Sparse)
    }

    #[test]
    fn top_level_union_rejected() {
        let err = reject_unsupported_widenings(&small_union_type()).unwrap_err();
        assert!(err.to_string().contains("union-typed"), "msg: {err}");
    }

    #[test]
    fn union_nested_inside_struct_rejected() {
        let struct_type = DataType::Struct(Fields::from(vec![
            Field::new("a", DataType::Int32, true),
            Field::new("u", small_union_type(), true),
        ]));
        let err = reject_unsupported_widenings(&struct_type).unwrap_err();
        assert!(err.to_string().contains("union-typed"), "msg: {err}");
    }

    #[test]
    fn union_nested_inside_list_rejected() {
        let list_of_union = DataType::List(Arc::new(Field::new("item", small_union_type(), true)));
        let err = reject_unsupported_widenings(&list_of_union).unwrap_err();
        assert!(err.to_string().contains("union-typed"), "msg: {err}");
    }

    /// Documents a known over-rejection that surfaces in `re_server`'s `add_layer` flow
    /// (`crates/store/re_server/src/store/dataset.rs`): when a new field differs from the
    /// current one by a non-Union sibling, `Schema::try_merge` would accept the pair cleanly
    /// and preserve any identical Union subtree untouched, but `reject_unsupported_widenings`
    /// walks only the new field and cannot distinguish "safe identical Union" from "unsafe
    /// widening Union" — so it rejects unconditionally.
    ///
    /// Closing this gap would require the gate to see both the current and new schemas and
    /// only reject at positions where the Union actually differs. If this test ever flips
    /// (i.e., the gate accepts), the aligner's Union handling must be re-verified end-to-end,
    /// including `arrow::array::new_null_array(DataType::Union(...), n)` behavior for the
    /// partition-missing-column null-pad path.
    #[test]
    fn union_over_rejected_when_only_a_sibling_widens() {
        use std::collections::HashMap;

        use arrow::datatypes::Schema;

        // Two structs with an identical Union child and a sibling whose nullability widens.
        let narrow_struct = DataType::Struct(Fields::from(vec![
            Field::new("a", DataType::Int32, false),
            Field::new("u", small_union_type(), true),
        ]));
        let wide_struct = DataType::Struct(Fields::from(vec![
            Field::new("a", DataType::Int32, true),
            Field::new("u", small_union_type(), true),
        ]));

        // `try_merge` is happy: `a` widens; the Union passes through unchanged.
        let lhs =
            Schema::new_with_metadata(vec![Field::new("s", narrow_struct, true)], HashMap::new());
        let rhs = Schema::new_with_metadata(
            vec![Field::new("s", wide_struct.clone(), true)],
            HashMap::new(),
        );
        Schema::try_merge([lhs, rhs])
            .expect("try_merge accepts: Union identical, only sibling widens");

        // The Rerun gate rejects, even though `try_merge` would not widen the Union.
        let err = reject_unsupported_widenings(&wide_struct).unwrap_err();
        assert!(err.to_string().contains("union-typed"), "msg: {err}");
    }

    #[test]
    fn plain_schema_accepted() {
        let schema = DataType::Struct(Fields::from(vec![
            Field::new("a", DataType::Int32, true),
            Field::new(
                "b",
                DataType::List(Arc::new(Field::new("item", DataType::Utf8, false))),
                true,
            ),
            Field::new(
                "c",
                DataType::Struct(Fields::from(vec![Field::new("d", DataType::Int64, false)])),
                true,
            ),
        ]));
        assert!(reject_unsupported_widenings(&schema).is_ok());
    }
}
