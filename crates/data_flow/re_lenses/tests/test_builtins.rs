#![expect(clippy::unwrap_used)] // Okay to use unwrap in tests

use std::sync::Arc;

use arrow::array::{
    Array, ArrayRef, ListArray, RecordBatch, RecordBatchOptions, StringBuilder, UInt32Array,
};
use arrow::datatypes::{Field, Schema};

use re_lenses::{Selector, default_runtime};
use re_lenses_core::SelectorError;

/// Wraps an array into a [`RecordBatch`] for readable snapshots.
struct DisplayRB<T: Array + Clone + 'static>(T);

impl<T: Array + Clone + 'static> std::fmt::Display for DisplayRB<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let array: ArrayRef = Arc::new(self.0.clone());
        let schema = Arc::new(Schema::new_with_metadata(
            vec![Field::new("col", array.data_type().clone(), true)],
            Default::default(),
        ));
        let rb =
            RecordBatch::try_new_with_options(schema, vec![array], &RecordBatchOptions::default())
                .unwrap();
        write!(f, "{}", re_arrow_util::format_record_batch(&rb))
    }
}

#[test]
fn selector_string_prefix_builtin() -> Result<(), SelectorError> {
    let array = ListArray::from_nested_iter::<StringBuilder, _, _, _>([
        Some([Some("world")]),
        Some([Some("rerun")]),
    ]);

    let selector = r#"string_prefix("hello_")"#.parse::<Selector>()?;
    let result = default_runtime()
        .execute_per_row(&selector, &array)?
        .unwrap();

    insta::assert_snapshot!(DisplayRB(result), @"
    ┌──────────────────┐
    │ col              │
    │ ---              │
    │ type: List(Utf8) │
    ╞══════════════════╡
    │ [hello_world]    │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [hello_rerun]    │
    └──────────────────┘
    ");

    Ok(())
}

/// Equal values get the same color, different values get different colors and nulls stay null.
#[test]
fn selector_color_for_builtin() -> Result<(), SelectorError> {
    let array = ListArray::from_nested_iter::<StringBuilder, _, _, _>([
        Some(vec![Some("walking"), Some("running")]),
        Some(vec![Some("walking"), None]),
    ]);

    let selector = "color_for()".parse::<Selector>()?;
    let result = default_runtime()
        .execute_per_row(&selector, &array)?
        .unwrap();

    let values = result
        .values()
        .as_any()
        .downcast_ref::<UInt32Array>()
        .unwrap();
    assert_eq!(values.len(), 4, "one color per input value");
    assert_eq!(
        values.value(0),
        values.value(2),
        "equal values get the same color"
    );
    assert_ne!(
        values.value(0),
        values.value(1),
        "different values get different colors"
    );
    assert!(values.is_null(3), "nulls stay null");

    Ok(())
}

/// Equal values in separate arrays get the same color.
#[test]
fn selector_color_for_across_arrays() -> Result<(), SelectorError> {
    let selector = "color_for()".parse::<Selector>()?;
    let color_of_first = |values: [&str; 2]| -> Result<u32, SelectorError> {
        let array = ListArray::from_nested_iter::<StringBuilder, _, _, _>([Some(
            values.map(Some).to_vec(),
        )]);
        let result = default_runtime()
            .execute_per_row(&selector, &array)?
            .unwrap();
        Ok(result
            .values()
            .as_any()
            .downcast_ref::<UInt32Array>()
            .unwrap()
            .value(0))
    };

    assert_eq!(
        color_of_first(["walking", "running"])?,
        color_of_first(["walking", "idle"])?,
        "the color only depends on the value"
    );

    Ok(())
}
