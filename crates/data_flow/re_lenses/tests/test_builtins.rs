#![expect(clippy::unwrap_used)] // Okay to use unwrap in tests

use std::sync::Arc;

use arrow::array::{Array, ArrayRef, ListArray, RecordBatch, RecordBatchOptions, StringBuilder};
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
