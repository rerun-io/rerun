#![expect(clippy::unwrap_used)]

use std::sync::Arc;

use arrow::array::{ArrayRef, Float64Array};
use re_chunk::{Chunk, RowId};
use re_lenses_core::combinators::Error;
use re_lenses_core::{ChunkExt as _, DynExpr, Selector};
use re_log_types::Timeline;
use re_sdk_types::ComponentDescriptor;

/// Doubles every float64 value.
fn double_values(source: &ArrayRef) -> Result<Option<ArrayRef>, Error> {
    let values = source
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| Error::TypeMismatch {
            expected: "Float64".into(),
            actual: source.data_type().clone(),
            context: "double_values".into(),
        })?;

    let doubled: Float64Array = values.iter().map(|v| v.map(|x| x * 2.0)).collect();
    Ok(Some(Arc::new(doubled)))
}

fn test_chunk() -> Chunk {
    let timeline = Timeline::new_sequence("tick");

    Chunk::builder("test/sensor")
        .with_row(
            RowId::new(),
            [(timeline, 0)],
            [(
                ComponentDescriptor::partial("value"),
                Arc::new(Float64Array::from(vec![1.0])) as ArrayRef,
            )],
        )
        .with_row(
            RowId::new(),
            [(timeline, 1)],
            [(
                ComponentDescriptor::partial("value"),
                Arc::new(Float64Array::from(vec![2.0])) as ArrayRef,
            )],
        )
        .build()
        .unwrap()
}

#[test]
fn apply_selector_doubles_values() {
    let chunk = test_chunk();
    insta::assert_snapshot!(format!("{:-240}", chunk), @r#"
    ┌────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                  │
    │ * entity_path: /test/sensor                                                                │
    │ * id: [**REDACTED**]                                                                       │
    │ * version: [**REDACTED**]                                                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌───────────────────────────────────────────────┬──────────────────┬─────────────────────┐ │
    │ │ RowId                                         ┆ tick             ┆ value               │ │
    │ │ ---                                           ┆ ---              ┆ ---                 │ │
    │ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64      ┆ type: List(Float64) │ │
    │ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: tick ┆ component: value    │ │
    │ │ ARROW:extension:name: TUID                    ┆ is_sorted: true  ┆ kind: data          │ │
    │ │ is_sorted: true                               ┆ kind: index      ┆                     │ │
    │ │ kind: control                                 ┆                  ┆                     │ │
    │ ╞═══════════════════════════════════════════════╪══════════════════╪═════════════════════╡ │
    │ │ row_[**REDACTED**]                            ┆ 0                ┆ [1.0]               │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                            ┆ 1                ┆ [2.0]               │ │
    │ └───────────────────────────────────────────────┴──────────────────┴─────────────────────┘ │
    └────────────────────────────────────────────────────────────────────────────────────────────┘
    "#);

    let selector: Selector<DynExpr> = Selector::parse(".").unwrap().pipe(double_values);

    let result = chunk.apply_selector("value".into(), &selector).unwrap();

    insta::assert_snapshot!(format!("{:-240}", result), @r#"
    ┌────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                  │
    │ * entity_path: /test/sensor                                                                │
    │ * id: [**REDACTED**]                                                                       │
    │ * version: [**REDACTED**]                                                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌───────────────────────────────────────────────┬──────────────────┬─────────────────────┐ │
    │ │ RowId                                         ┆ tick             ┆ value               │ │
    │ │ ---                                           ┆ ---              ┆ ---                 │ │
    │ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64      ┆ type: List(Float64) │ │
    │ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: tick ┆ component: value    │ │
    │ │ ARROW:extension:name: TUID                    ┆ is_sorted: true  ┆ kind: data          │ │
    │ │ is_sorted: true                               ┆ kind: index      ┆                     │ │
    │ │ kind: control                                 ┆                  ┆                     │ │
    │ ╞═══════════════════════════════════════════════╪══════════════════╪═════════════════════╡ │
    │ │ row_[**REDACTED**]                            ┆ 0                ┆ [2.0]               │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                            ┆ 1                ┆ [4.0]               │ │
    │ └───────────────────────────────────────────────┴──────────────────┴─────────────────────┘ │
    └────────────────────────────────────────────────────────────────────────────────────────────┘
    "#);
}
