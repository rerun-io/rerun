use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef, GenericListArray, OffsetSizeTrait, RecordBatch, RecordBatchOptions,
    StructArray, new_null_array,
};
use arrow::datatypes::{DataType, Field, FieldRef, Fields, Schema};
use arrow::error::ArrowError;

use re_arrow_util::format_field_datatype;

/// Align a [`RecordBatch`] to a target [`Schema`], widening nested types where possible.
///
/// # Schema-widening contract (RR-4429)
///
/// The table below lists only the cases arrow's `Field::try_merge` used at registration
/// would accept. Anything `try_merge` rejects cannot reach here.
///
/// | Case                                        | Read-side widener |
/// |---------------------------------------------|-------------------|
/// | Struct child added (`{a,b}` → `{a,b,c}`)    | adapt (null-pad)  |
/// | Nullability widening (non-null → nullable)  | adapt (re-wrap)   |
/// | `DataType::Null` → typed, at any depth      | adapt (typed null-array) |
/// | `List` / `LargeList` inner widened          | adapt (recurse)   |
/// | Identical types                             | fast-path pass through |
/// | `Union`                                     | **reject** at registration (aligner gap) |
///
/// Types in arrow-schema's leaf-equality bucket (`FixedSizeList`, primitives, `Dictionary`, `Map`,
/// decimals, `RunEndEncoded`, etc.) cannot reach the aligner in non-identical shape — `Field::try_merge`
/// rejects any non-identical pair before the aligner runs. So the aligner's only job for those types
/// is to pass them through when identical, which the fast-path below handles.
///
/// `Union` is the one exception: `try_merge` *does* recursively widen Union children, which the
/// aligner has no branch for. It's rejected at registration by
/// [`re_arrow_util::reject_unsupported_widenings`] so it never reaches any of the above logic.
#[tracing::instrument(level = "trace", skip_all)]
pub fn align_record_batch_to_schema(
    batch: &RecordBatch,
    target_schema: &Arc<Schema>,
) -> Result<RecordBatch, ArrowError> {
    let num_rows = batch.num_rows();
    let mut aligned = Vec::with_capacity(target_schema.fields().len());

    for field in target_schema.fields() {
        let col = match batch.schema().column_with_name(field.name()) {
            Some((idx, _)) => widen_array_to_field(batch.column(idx), field, field.name())?,
            None => new_null_array(field.data_type(), num_rows),
        };
        aligned.push(col);
    }

    RecordBatch::try_new_with_options(
        target_schema.clone(),
        aligned,
        &RecordBatchOptions::new().with_row_count(Some(num_rows)),
    )
}

/// Widen `array` to match the shape of `target`'s data type.
///
/// `path` is a dotted breadcrumb used only for error messages
fn widen_array_to_field(
    array: &ArrayRef,
    target: &FieldRef,
    path: &str,
) -> Result<ArrayRef, ArrowError> {
    // A `Null`-typed source column converts to a typed null-array of the target type.
    if matches!(array.data_type(), DataType::Null) {
        return Ok(new_null_array(target.data_type(), array.len()));
    }

    // Identical data types pass through (schema-widening contract row).
    // `DataType` equality is structural and recursive (including inner `Field` nullability),
    // so this correctly shortcuts only the cases where no widening is needed.
    if array.data_type() == target.data_type() {
        return Ok(array.clone());
    }

    match target.data_type() {
        DataType::Struct(t_fields) => {
            let t_fields = t_fields.clone();
            if !matches!(array.data_type(), DataType::Struct(_)) {
                return Err(type_differs_error(path, target, array.data_type()));
            }
            widen_struct(array, &t_fields, path)
        }
        DataType::List(t_inner) => {
            let t_inner = t_inner.clone();
            let DataType::List(_) = array.data_type() else {
                return Err(type_differs_error(path, target, array.data_type()));
            };
            widen_list_like::<i32>(array, &t_inner, path)
        }
        DataType::LargeList(t_inner) => {
            let t_inner = t_inner.clone();
            let DataType::LargeList(_) = array.data_type() else {
                return Err(type_differs_error(path, target, array.data_type()));
            };
            widen_list_like::<i64>(array, &t_inner, path)
        }
        // `FixedSizeList` and other leaf-equality types are caught by the fast-path above when
        // identical; `try_merge` rejects any non-identical shape, so reaching this arm means an
        // upstream invariant violated the contract.
        _ => Err(type_differs_error(path, target, array.data_type())),
    }
}

fn widen_struct(
    array: &ArrayRef,
    target_fields: &Fields,
    path: &str,
) -> Result<ArrayRef, ArrowError> {
    let struct_array = array
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| {
            // defensive
            schema_mismatch(
                path,
                &format!("expected struct array, got {}", array.data_type()),
            )
        })?;
    let struct_len = struct_array.len();

    // Assumes target children are a superset of source children (guaranteed by
    // `Schema::try_merge`, which only ever widens).
    let mut widened_children = Vec::with_capacity(target_fields.len());
    for t_child in target_fields {
        let child_path = format!("{path}.{}", t_child.name());
        let child = if let Some(source_col) = struct_array.column_by_name(t_child.name()) {
            widen_array_to_field(source_col, t_child, &child_path)?
        } else {
            new_null_array(t_child.data_type(), struct_len)
        };
        widened_children.push(child);
    }

    Ok(Arc::new(StructArray::try_new(
        target_fields.clone(),
        widened_children,
        struct_array.nulls().cloned(),
    )?) as ArrayRef)
}

fn widen_list_like<O: OffsetSizeTrait>(
    array: &ArrayRef,
    target_inner: &FieldRef,
    path: &str,
) -> Result<ArrayRef, ArrowError> {
    let list_array = array
        .as_any()
        .downcast_ref::<GenericListArray<O>>()
        .ok_or_else(|| {
            schema_mismatch(
                path,
                &format!("expected list array, got {}", array.data_type()),
            )
        })?;

    let item_path = format!("{path}.{}", target_inner.name());
    let widened_values = widen_array_to_field(list_array.values(), target_inner, &item_path)?;

    Ok(Arc::new(GenericListArray::<O>::try_new(
        target_inner.clone(),
        list_array.offsets().clone(),
        widened_values,
        list_array.nulls().cloned(),
    )?) as ArrayRef)
}

#[inline]
fn type_differs_error(path: &str, target: &Field, actual: &DataType) -> ArrowError {
    schema_mismatch(
        path,
        &format!(
            "type differs (expected {}, got {actual})",
            format_field_datatype(target),
        ),
    )
}

#[inline]
fn schema_mismatch(path: &str, detail: &str) -> ArrowError {
    ArrowError::SchemaError(format!("rerun schema mismatch at `{path}`: {detail}"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use arrow::array::{ArrayRef, Int32Array, Int64Array, ListArray, StringArray, StructArray};
    use arrow::buffer::OffsetBuffer;
    use arrow::datatypes::{DataType, Field, Fields, Schema};

    fn new_schema(fields: Vec<Field>) -> Schema {
        let meta = HashMap::with_capacity(0);
        Schema::new_with_metadata(fields, meta)
    }

    /// Wrapper around `RecordBatch::try_new_with_options` so tests match the project's lint
    /// policy without each call-site specifying row-count explicitly.
    fn rb(schema: Arc<Schema>, columns: Vec<ArrayRef>) -> RecordBatch {
        let num_rows = columns.first().map_or(0, |c| c.len());
        RecordBatch::try_new_with_options(
            schema,
            columns,
            &RecordBatchOptions::new().with_row_count(Some(num_rows)),
        )
        .unwrap()
    }

    #[test]
    fn align_missing_top_level_column_null_pads() {
        let target = Arc::new(new_schema(vec![
            Field::new("a", DataType::Int32, true),
            Field::new("b", DataType::Utf8, true),
        ]));
        let batch = rb(
            Arc::new(new_schema(vec![Field::new("a", DataType::Int32, true)])),
            vec![Arc::new(Int32Array::from(vec![1, 2, 3])) as ArrayRef],
        );

        let aligned = align_record_batch_to_schema(&batch, &target).unwrap();
        assert_eq!(aligned.num_rows(), 3);
        assert_eq!(aligned.num_columns(), 2);
        assert_eq!(aligned.column(1).null_count(), 3);
    }

    #[test]
    fn align_widens_struct_with_missing_child() {
        let target_struct = DataType::Struct(
            vec![
                Field::new("a", DataType::Int32, false),
                Field::new("b", DataType::Int32, false),
                Field::new("c", DataType::Int32, true),
            ]
            .into(),
        );
        let source_struct = StructArray::try_new(
            Fields::from(vec![
                Field::new("a", DataType::Int32, false),
                Field::new("b", DataType::Int32, false),
            ]),
            vec![
                Arc::new(Int32Array::from(vec![1, 2])) as ArrayRef,
                Arc::new(Int32Array::from(vec![10, 20])) as ArrayRef,
            ],
            None,
        )
        .unwrap();

        let target = Arc::new(new_schema(vec![Field::new("s", target_struct, true)]));
        let batch = rb(
            Arc::new(new_schema(vec![Field::new(
                "s",
                source_struct.data_type().clone(),
                true,
            )])),
            vec![Arc::new(source_struct) as ArrayRef],
        );

        let aligned = align_record_batch_to_schema(&batch, &target).unwrap();
        let widened = aligned
            .column(0)
            .as_any()
            .downcast_ref::<StructArray>()
            .expect("struct");
        assert_eq!(widened.num_columns(), 3);
        assert_eq!(widened.column_by_name("c").unwrap().null_count(), 2);
    }

    #[test]
    fn align_widens_list_inner_nullability_non_null_to_nullable() {
        let source_inner_non_null = Arc::new(Field::new("item", DataType::Int32, false));
        let target_inner_nullable = Arc::new(Field::new("item", DataType::Int32, true));

        let values = Int32Array::from(vec![1, 2, 3]);
        let source_list = ListArray::new(
            source_inner_non_null.clone(),
            OffsetBuffer::new(vec![0i32, 3].into()),
            Arc::new(values),
            None,
        );

        let target = Arc::new(new_schema(vec![Field::new(
            "col",
            DataType::List(target_inner_nullable),
            true,
        )]));
        let batch = rb(
            Arc::new(new_schema(vec![Field::new(
                "col",
                DataType::List(source_inner_non_null),
                true,
            )])),
            vec![Arc::new(source_list) as ArrayRef],
        );

        let aligned = align_record_batch_to_schema(&batch, &target).unwrap();
        assert_eq!(aligned.num_rows(), 1);
    }

    #[test]
    fn align_primitive_mismatch_errors_compactly() {
        let target = Arc::new(new_schema(vec![Field::new("a", DataType::Int64, false)]));
        let batch = rb(
            Arc::new(new_schema(vec![Field::new("a", DataType::Int32, false)])),
            vec![Arc::new(Int32Array::from(vec![1, 2, 3])) as ArrayRef],
        );

        let err = align_record_batch_to_schema(&batch, &target).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("rerun schema mismatch at `a`"), "msg: {msg}");
        assert!(msg.contains("type differs"), "msg: {msg}");
        assert!(msg.contains("Int64"), "msg: {msg}");
        assert!(msg.contains("Int32"), "msg: {msg}");
        // Compact formatting — no Field-struct Debug spew.
        assert!(!msg.contains("Field {"), "msg: {msg}");
        assert!(!msg.contains("dict_id"), "msg: {msg}");
    }

    #[test]
    fn align_deeply_nested_path_in_error() {
        let target_inner_struct = DataType::Struct(
            vec![
                Field::new("a", DataType::Int32, false),
                Field::new("b", DataType::Int32, true),
            ]
            .into(),
        );
        let source_inner_struct =
            DataType::Struct(vec![Field::new("a", DataType::Int64, false)].into());

        let target_outer = DataType::Struct(
            vec![Field::new(
                "outer_list",
                DataType::List(Arc::new(Field::new("item", target_inner_struct, true))),
                true,
            )]
            .into(),
        );
        let source_outer = DataType::Struct(
            vec![Field::new(
                "outer_list",
                DataType::List(Arc::new(Field::new(
                    "item",
                    source_inner_struct.clone(),
                    true,
                ))),
                true,
            )]
            .into(),
        );

        let inner = StructArray::try_new(
            Fields::from(vec![Field::new("a", DataType::Int64, false)]),
            vec![Arc::new(Int64Array::from(vec![1])) as ArrayRef],
            None,
        )
        .unwrap();
        let inner_list = ListArray::new(
            Arc::new(Field::new("item", source_inner_struct, true)),
            OffsetBuffer::new(vec![0i32, 1].into()),
            Arc::new(inner),
            None,
        );
        let outer = StructArray::try_new(
            Fields::from(vec![Field::new(
                "outer_list",
                DataType::List(Arc::new(Field::new(
                    "item",
                    DataType::Struct(vec![Field::new("a", DataType::Int64, false)].into()),
                    true,
                ))),
                true,
            )]),
            vec![Arc::new(inner_list) as ArrayRef],
            None,
        )
        .unwrap();

        let target = Arc::new(new_schema(vec![Field::new("top", target_outer, true)]));
        let batch = rb(
            Arc::new(new_schema(vec![Field::new("top", source_outer, true)])),
            vec![Arc::new(outer) as ArrayRef],
        );

        let err = align_record_batch_to_schema(&batch, &target).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("at `top.outer_list.item.a`"), "msg: {msg}");
        assert!(msg.contains("type differs"), "msg: {msg}");
    }

    #[test]
    fn align_null_typed_source_becomes_typed_null_at_any_depth() {
        let target = Arc::new(new_schema(vec![Field::new("a", DataType::Int32, true)]));
        let batch = rb(
            Arc::new(new_schema(vec![Field::new("a", DataType::Null, true)])),
            vec![new_null_array(&DataType::Null, 2)],
        );
        let aligned = align_record_batch_to_schema(&batch, &target).unwrap();
        assert_eq!(aligned.column(0).data_type(), &DataType::Int32);
        assert_eq!(aligned.column(0).null_count(), 2);
    }

    #[test]
    fn align_already_matching_short_circuits() {
        let target = Arc::new(new_schema(vec![Field::new("a", DataType::Utf8, false)]));
        let batch = rb(
            target.clone(),
            vec![Arc::new(StringArray::from(vec!["x", "y"])) as ArrayRef],
        );
        let aligned = align_record_batch_to_schema(&batch, &target).unwrap();
        assert_eq!(aligned.num_rows(), 2);
    }
}

/// Executable documentation of arrow's `Schema::try_merge` expectations.
///
/// The aligner above relies on these invariants: it only adapts cases `try_merge` actually
/// emits, and every other shape is assumed unreachable. If arrow-rs ever changes one of these
/// rules (e.g. starts widening FSL inner fields, or allows dictionary key promotion), the
/// corresponding test here fails and points at exactly which aligner assumption needs
/// revisiting before we silently accept inputs the aligner can't handle.
#[cfg(test)]
mod try_merge_invariants {
    use std::sync::Arc;

    use std::collections::HashMap;

    use arrow::datatypes::{DataType, Field, Schema};

    fn schema_of(field: Field) -> Schema {
        Schema::new_with_metadata(vec![field], HashMap::with_capacity(0))
    }

    fn try_merge_fields(a: Field, b: Field) -> Result<Schema, arrow::error::ArrowError> {
        Schema::try_merge([schema_of(a), schema_of(b)])
    }

    // ---- FixedSizeList: leaf-equality, never widened by `try_merge` ---------------------------

    #[test]
    fn try_merge_rejects_fsl_inner_nullability_drift() {
        let lhs = Field::new(
            "x",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Int32, false)), 3),
            true,
        );
        let rhs = Field::new(
            "x",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Int32, true)), 3),
            true,
        );
        assert!(try_merge_fields(lhs, rhs).is_err());
    }

    #[test]
    fn try_merge_rejects_fsl_inner_type_drift() {
        let lhs = Field::new(
            "x",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Int32, true)), 3),
            true,
        );
        let rhs = Field::new(
            "x",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Int64, true)), 3),
            true,
        );
        assert!(try_merge_fields(lhs, rhs).is_err());
    }

    #[test]
    fn try_merge_rejects_fsl_length_drift() {
        let lhs = Field::new(
            "x",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Int32, true)), 3),
            true,
        );
        let rhs = Field::new(
            "x",
            DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Int32, true)), 4),
            true,
        );
        assert!(try_merge_fields(lhs, rhs).is_err());
    }

    #[test]
    fn try_merge_accepts_identical_fsl() {
        let dt = DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Int32, true)), 3);
        let lhs = Field::new("x", dt.clone(), true);
        let rhs = Field::new("x", dt.clone(), true);
        let merged = try_merge_fields(lhs, rhs).expect("identical FSLs must merge");
        assert_eq!(merged.field(0).data_type(), &dt);
    }

    // ---- Dictionary: leaf-equality, never widened by `try_merge` ------------------------------

    #[test]
    fn try_merge_rejects_dictionary_key_drift() {
        let lhs = Field::new(
            "x",
            DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8)),
            true,
        );
        let rhs = Field::new(
            "x",
            DataType::Dictionary(Box::new(DataType::Int64), Box::new(DataType::Utf8)),
            true,
        );
        assert!(try_merge_fields(lhs, rhs).is_err());
    }

    #[test]
    fn try_merge_rejects_dictionary_value_drift() {
        let lhs = Field::new(
            "x",
            DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8)),
            true,
        );
        let rhs = Field::new(
            "x",
            DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::LargeUtf8)),
            true,
        );
        assert!(try_merge_fields(lhs, rhs).is_err());
    }

    // ---- Nullability: widens toward nullable, never narrows -----------------------------------

    #[test]
    fn try_merge_widens_nullability_never_narrows() {
        let non_null = Field::new("x", DataType::Int32, false);
        let nullable = Field::new("x", DataType::Int32, true);
        let merged = try_merge_fields(non_null, nullable).expect("mixed nullability must merge");
        assert!(
            merged.field(0).is_nullable(),
            "merged field must be nullable (widening direction)"
        );
    }

    // ---- List inner nullability widening: the one nested case the aligner actively uses -------

    #[test]
    fn try_merge_widens_list_inner_nullability() {
        let lhs = Field::new(
            "x",
            DataType::List(Arc::new(Field::new("item", DataType::Int32, false))),
            true,
        );
        let rhs = Field::new(
            "x",
            DataType::List(Arc::new(Field::new("item", DataType::Int32, true))),
            true,
        );
        let merged = try_merge_fields(lhs, rhs).expect("list inner widening must merge");
        let DataType::List(inner) = merged.field(0).data_type() else {
            panic!("expected list");
        };
        assert!(inner.is_nullable(), "merged list inner must be nullable");
    }

    // ---- Null → typed: other key case the aligner actively adapts -----------------------------

    #[test]
    fn try_merge_widens_null_to_typed() {
        let typed = Field::new("x", DataType::Int32, false);
        let null = Field::new("x", DataType::Null, true);
        let merged = try_merge_fields(typed, null).expect("Null → typed must merge");
        assert_eq!(merged.field(0).data_type(), &DataType::Int32);
        assert!(
            merged.field(0).is_nullable(),
            "Null contributes nullability"
        );
    }

    // ---- Leaf-equality composites are opaque to *inner* widening ------------------------------
    //
    // These tests are the strong invariant the aligner relies on: when an inner `Field` or
    // `DataType` *would* widen if placed at the top level (per the `widens_*` tests above),
    // wrapping it in `Map` / `Dictionary` / `FixedSizeList` makes `try_merge` reject the pair
    // instead of recursing. This is what lets the aligner skip writing inner-widening logic for
    // these composite types.

    use arrow::datatypes::Fields;

    fn map_type_with_value_field(value: Field) -> DataType {
        let entries = Field::new(
            "entries",
            DataType::Struct(Fields::from(vec![
                Field::new("key", DataType::Utf8, false),
                value,
            ])),
            false,
        );
        DataType::Map(Arc::new(entries), false)
    }

    #[test]
    fn try_merge_rejects_map_value_nullability_widening() {
        // A nullability drift on the value field: at top level this would widen (see
        // `try_merge_widens_nullability_never_narrows`). Wrapped in Map, try_merge rejects.
        let lhs = Field::new(
            "m",
            map_type_with_value_field(Field::new("value", DataType::Int32, false)),
            true,
        );
        let rhs = Field::new(
            "m",
            map_type_with_value_field(Field::new("value", DataType::Int32, true)),
            true,
        );
        assert!(try_merge_fields(lhs, rhs).is_err());
    }

    #[test]
    fn try_merge_rejects_map_value_struct_child_addition() {
        // A struct-child addition on the value field: at top level this would widen (struct
        // child-addition is in the aligner's contract). Wrapped in Map, try_merge rejects.
        let v_narrow = DataType::Struct(Fields::from(vec![Field::new("a", DataType::Int32, true)]));
        let v_wide = DataType::Struct(Fields::from(vec![
            Field::new("a", DataType::Int32, true),
            Field::new("b", DataType::Int32, true),
        ]));
        let lhs = Field::new(
            "m",
            map_type_with_value_field(Field::new("value", v_narrow, true)),
            true,
        );
        let rhs = Field::new(
            "m",
            map_type_with_value_field(Field::new("value", v_wide, true)),
            true,
        );
        assert!(try_merge_fields(lhs, rhs).is_err());
    }

    #[test]
    fn try_merge_rejects_dictionary_value_struct_child_addition() {
        // Same shape as the Map test, for Dictionary: a widenable struct on the values side.
        let v_narrow = DataType::Struct(Fields::from(vec![Field::new("a", DataType::Int32, true)]));
        let v_wide = DataType::Struct(Fields::from(vec![
            Field::new("a", DataType::Int32, true),
            Field::new("b", DataType::Int32, true),
        ]));
        let lhs = Field::new(
            "d",
            DataType::Dictionary(Box::new(DataType::Int32), Box::new(v_narrow)),
            true,
        );
        let rhs = Field::new(
            "d",
            DataType::Dictionary(Box::new(DataType::Int32), Box::new(v_wide)),
            true,
        );
        assert!(try_merge_fields(lhs, rhs).is_err());
    }

    #[test]
    fn try_merge_rejects_fsl_inner_struct_child_addition() {
        // Same shape as above, for FixedSizeList: a widenable struct as the FSL item type.
        let inner_narrow =
            DataType::Struct(Fields::from(vec![Field::new("a", DataType::Int32, true)]));
        let inner_wide = DataType::Struct(Fields::from(vec![
            Field::new("a", DataType::Int32, true),
            Field::new("b", DataType::Int32, true),
        ]));
        let lhs = Field::new(
            "f",
            DataType::FixedSizeList(Arc::new(Field::new("item", inner_narrow, true)), 3),
            true,
        );
        let rhs = Field::new(
            "f",
            DataType::FixedSizeList(Arc::new(Field::new("item", inner_wide, true)), 3),
            true,
        );
        assert!(try_merge_fields(lhs, rhs).is_err());
    }
}
