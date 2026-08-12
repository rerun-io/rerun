// Tests for the `pack(…)` selector, which packs 1:1 paths into a `FixedSizeList`.
//
// NOTE: nullability is **type-driven**, not data-driven. A field declared nullable in the
// schema requires `!` even when the current batch happens to contain no nulls. This is why
// the examples below mark every nullable path with `!`, regardless of its actual contents.
#![expect(clippy::unwrap_used)]

use std::sync::Arc;

use arrow::array::{Array, FixedSizeListArray, Float64Array, Int64Array, ListArray, StructArray};
use arrow::buffer::OffsetBuffer;
use arrow::datatypes::{DataType, Field, Fields};

use re_lenses_core::{Selector, SelectorError as Error};

/// Build a non-null `Struct { x, y }` column with per-field nullability (`[x, y]`) and contents.
fn xy(nullable: [bool; 2], x: Int64Array, y: Int64Array) -> StructArray {
    let fields = Fields::from(vec![
        Field::new("x", DataType::Int64, nullable[0]),
        Field::new("y", DataType::Int64, nullable[1]),
    ]);
    StructArray::new(fields, vec![Arc::new(x), Arc::new(y)], None)
}

/// Downcast a result array to a `FixedSizeListArray`.
fn as_fsl(array: &Arc<dyn Array>) -> &FixedSizeListArray {
    array.as_any().downcast_ref::<FixedSizeListArray>().unwrap()
}

/// Assert that row `row` of a `FixedSizeList<Int64>` is valid and equals `expected`.
fn assert_row(fsl: &FixedSizeListArray, row: usize, expected: &[i64]) {
    assert!(fsl.is_valid(row), "expected row {row} to be valid");
    let values = fsl.value(row);
    let values = values.as_any().downcast_ref::<Int64Array>().unwrap();
    let actual: Vec<i64> = (0..values.len()).map(|i| values.value(i)).collect();
    assert_eq!(actual, expected, "row {row} values");
}

#[test]
fn pack_all_non_nullable() -> Result<(), Error> {
    // No path is nullable: no `!` needed, and the result is a non-nullable FixedSizeList.
    let col = xy(
        [false, false],
        Int64Array::from(vec![0, 0, 0, 0]),
        Int64Array::from(vec![1, 1, 1, 1]),
    );

    let result = "pack(.x, .y)"
        .parse::<Selector>()?
        .execute(Arc::new(col))?
        .unwrap();

    let fsl = as_fsl(&result);
    assert_eq!(fsl.len(), 4);
    assert_eq!(fsl.value_length(), 2);
    assert_eq!(fsl.null_count(), 0);

    // The element field is non-nullable when no path is nullable.
    let DataType::FixedSizeList(field, 2) = fsl.data_type() else {
        panic!("expected FixedSizeList[2], got {}", fsl.data_type());
    };
    assert!(!field.is_nullable());

    for row in 0..4 {
        assert_row(fsl, row, &[0, 1]);
    }

    Ok(())
}

/// Example 1: `.x` nullable but dense, `.y` nullable with a null at row 2.
#[test]
fn pack_example_1() -> Result<(), Error> {
    let col = xy(
        [true, true],
        Int64Array::from(vec![Some(0), Some(0), Some(0), Some(0)]),
        Int64Array::from(vec![Some(1), Some(1), None, Some(1)]),
    );

    let result = "pack(.x!, .y!)"
        .parse::<Selector>()?
        .execute(Arc::new(col))?
        .unwrap();

    let fsl = as_fsl(&result);
    assert_row(fsl, 0, &[0, 1]);
    assert_row(fsl, 1, &[0, 1]);
    assert!(fsl.is_null(2)); // .y null shadows the whole entry
    assert_row(fsl, 3, &[0, 1]);

    Ok(())
}

/// Example 2: `.x` **non-nullable**, `.y` nullable with a null at row 2.
/// Only `.y` needs `!`; `.x`'s value at the null row is shadowed.
#[test]
fn pack_example_2_mixed_nullability() -> Result<(), Error> {
    let col = xy(
        [false, true],
        Int64Array::from(vec![0, 0, 0, 0]),
        Int64Array::from(vec![Some(1), Some(1), None, Some(1)]),
    );

    let result = "pack(.x, .y!)"
        .parse::<Selector>()?
        .execute(Arc::new(col))?
        .unwrap();

    let fsl = as_fsl(&result);
    assert_row(fsl, 0, &[0, 1]);
    assert_row(fsl, 1, &[0, 1]);
    assert!(fsl.is_null(2));
    assert_row(fsl, 3, &[0, 1]);

    Ok(())
}

/// Example 4: both paths nullable, nulls in different rows; both must be acknowledged.
#[test]
fn pack_example_4_disjoint_nulls() -> Result<(), Error> {
    let col = xy(
        [true, true],
        Int64Array::from(vec![Some(0), None, Some(0), Some(0)]),
        Int64Array::from(vec![Some(1), Some(1), None, Some(1)]),
    );

    let result = "pack(.x!, .y!)"
        .parse::<Selector>()?
        .execute(Arc::new(col))?
        .unwrap();

    let fsl = as_fsl(&result);
    assert_row(fsl, 0, &[0, 1]);
    assert!(fsl.is_null(1)); // .x null
    assert!(fsl.is_null(2)); // .y null
    assert_row(fsl, 3, &[0, 1]);

    // The element field is nullable when any path is nullable.
    let DataType::FixedSizeList(field, 2) = fsl.data_type() else {
        panic!("expected FixedSizeList[2], got {}", fsl.data_type());
    };
    assert!(field.is_nullable());

    Ok(())
}

#[test]
fn pack_nullable_path_without_bang_errors() -> Result<(), Error> {
    // `.x` is nullable-typed; omitting `!` is an error even though `.y` is acknowledged.
    let col = xy(
        [true, true],
        Int64Array::from(vec![Some(0), Some(0), Some(0), Some(0)]),
        Int64Array::from(vec![Some(1), Some(1), None, Some(1)]),
    );

    let err = "pack(.x, .y!)"
        .parse::<Selector>()?
        .execute(Arc::new(col))
        .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains(".x"),
        "error should name the offending path: {msg}"
    );
    assert!(
        msg.contains("acknowledged with `!`"),
        "error should explain the `!` requirement: {msg}"
    );

    Ok(())
}

#[test]
fn pack_type_mismatch_errors() -> Result<(), Error> {
    let fields = Fields::from(vec![
        Field::new("x", DataType::Int64, false),
        Field::new("y", DataType::Float64, false),
    ]);
    let col = StructArray::new(
        fields,
        vec![
            Arc::new(Int64Array::from(vec![0, 0])),
            Arc::new(Float64Array::from(vec![1.0, 1.0])),
        ],
        None,
    );

    let err = "pack(.x, .y)"
        .parse::<Selector>()?
        .execute(Arc::new(col))
        .unwrap_err();

    assert!(
        err.to_string().contains("same datatype"),
        "expected a datatype-mismatch error, got: {err}"
    );

    Ok(())
}

#[test]
fn pack_non_scalar_path_errors() {
    // `map(…)` is not 1:1 and cannot fill a fixed-size slot. The restriction is structural,
    // so it is caught at parse time — before any data is touched.
    let err = "pack(map(.x), .y)".parse::<Selector>().unwrap_err();

    assert!(
        err.to_string().contains("scalar navigation"),
        "expected a non-scalar-path error, got: {err}"
    );
}

#[test]
fn pack_per_row_composition() -> Result<(), Error> {
    // Packing inside a per-row context yields `List<FixedSizeList<Int64>[2]>`.
    let fields = Fields::from(vec![
        Field::new("x", DataType::Int64, false),
        Field::new("y", DataType::Int64, false),
    ]);
    let structs = StructArray::new(
        fields.clone(),
        vec![
            Arc::new(Int64Array::from(vec![1, 2, 3])),
            Arc::new(Int64Array::from(vec![4, 5, 6])),
        ],
        None,
    );
    let list = ListArray::new(
        Arc::new(Field::new_list_field(DataType::Struct(fields), false)),
        OffsetBuffer::from_lengths([2, 1]),
        Arc::new(structs),
        None,
    );

    let result = "pack(.x, .y)"
        .parse::<Selector>()?
        .execute_per_row(&list)?
        .unwrap();

    assert_eq!(result.len(), 2);
    assert!(
        matches!(result.value_type(), DataType::FixedSizeList(_, 2)),
        "expected List<FixedSizeList[2]>, got {}",
        result.data_type()
    );

    Ok(())
}
