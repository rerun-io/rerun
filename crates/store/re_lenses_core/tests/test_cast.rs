//! Tests for opt-in auto-casting of derive lens output columns ([`CastTo`]).

#![expect(clippy::unwrap_used)]

mod util;

use std::sync::Arc;

use arrow::array::{
    ArrayRef, FixedSizeListArray, Float64Array, Int64Array, ListArray, StringArray,
};
use arrow::datatypes::{DataType, Field, Float64Type};

use re_chunk::{Chunk, RowId};
use re_lenses_core::function_registry::FunctionRegistry;
use re_lenses_core::{CastTo, ChunkExt as _, Lens, LensRuntimeError, Runtime, Selector};
use re_log_types::Timeline;
use re_sdk_types::ComponentDescriptor;
use re_sdk_types::archetypes::{Points3D, Scalars};

use util::DisplayRB;

fn runtime() -> Runtime {
    Runtime::new(Arc::new(FunctionRegistry::new()))
}

/// Builds a chunk on `test/sensor` with a single `value` component column, one row per array.
fn chunk_with_value_column(rows: Vec<ArrayRef>) -> Chunk {
    let timeline = Timeline::new_sequence("tick");
    let mut builder = Chunk::builder("test/sensor");
    for (i, array) in rows.into_iter().enumerate() {
        builder = builder.with_row(
            RowId::new(),
            [(timeline, i64::try_from(i).unwrap())],
            [(ComponentDescriptor::partial("value"), array)],
        );
    }
    builder.build().unwrap()
}

/// The produced column for `component`, found across the output chunks.
///
/// Returned as a standalone [`ListArray`] so it can be snapshotted via [`DisplayRB`],
/// which renders the element type, values, and nulls together.
fn output_column(chunks: &[Chunk], component: re_chunk::ComponentIdentifier) -> ListArray {
    chunks
        .iter()
        .find_map(|chunk| chunk.components().get(component))
        .expect("output component not found in any produced chunk")
        .list_array
        .clone()
}

#[test]
fn auto_casts_to_canonical_scalar_float64() {
    let descr = Scalars::descriptor_scalars();
    let chunk = chunk_with_value_column(vec![
        Arc::new(Int64Array::from(vec![1])) as ArrayRef,
        Arc::new(Int64Array::from(vec![2])) as ArrayRef,
    ]);

    let lens = Lens::derive("value")
        .output_entity("out")
        .to_component_with_cast(descr.clone(), Selector::parse(".").unwrap(), CastTo::Auto)
        .build()
        .unwrap();

    let chunks = chunk.apply_lenses(&[lens], &runtime()).unwrap();

    // Scalar's canonical Arrow type is Float64, so the Int64 input is cast (values preserved).
    insta::assert_snapshot!(DisplayRB(output_column(&chunks, descr.component)), @r"
    ┌─────────────────────┐
    │ col                 │
    │ ---                 │
    │ type: List(Float64) │
    ╞═════════════════════╡
    │ [1.0]               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [2.0]               │
    └─────────────────────┘
    ");
}

#[test]
fn to_component_defaults_to_no_cast() {
    let descr = Scalars::descriptor_scalars();
    let chunk = chunk_with_value_column(vec![Arc::new(Int64Array::from(vec![1])) as ArrayRef]);

    let lens = Lens::derive("value")
        .output_entity("out")
        .to_component(descr.clone(), Selector::parse(".").unwrap())
        .build()
        .unwrap();

    let chunks = chunk.apply_lenses(&[lens], &runtime()).unwrap();

    // Without a cast the Int64 input is emitted as-is, even though Scalar is Float64.
    insta::assert_snapshot!(DisplayRB(output_column(&chunks, descr.component)), @r"
    ┌───────────────────┐
    │ col               │
    │ ---               │
    │ type: List(Int64) │
    ╞═══════════════════╡
    │ [1]               │
    └───────────────────┘
    ");
}

#[test]
fn explicit_type_cast_to_float32() {
    let descr = Scalars::descriptor_scalars();
    let chunk = chunk_with_value_column(vec![Arc::new(Float64Array::from(vec![1.5])) as ArrayRef]);

    let lens = Lens::derive("value")
        .output_entity("out")
        .to_component_with_cast(
            descr.clone(),
            Selector::parse(".").unwrap(),
            CastTo::Type(DataType::Float32),
        )
        .build()
        .unwrap();

    let chunks = chunk.apply_lenses(&[lens], &runtime()).unwrap();

    insta::assert_snapshot!(DisplayRB(output_column(&chunks, descr.component)), @r"
    ┌─────────────────────┐
    │ col                 │
    │ ---                 │
    │ type: List(Float32) │
    ╞═════════════════════╡
    │ [1.5]               │
    └─────────────────────┘
    ");
}

#[test]
fn auto_casts_fixed_size_list_f64_to_f32() {
    // Points3D positions are `FixedSizeList<Float32, 3>`; the input is the f64 equivalent.
    let descr = Points3D::descriptor_positions();
    let input = FixedSizeListArray::from_iter_primitive::<Float64Type, _, _>(
        [Some([Some(1.0), Some(2.0), Some(3.0)])],
        3,
    );
    let chunk = chunk_with_value_column(vec![Arc::new(input) as ArrayRef]);

    let lens = Lens::derive("value")
        .output_entity("out")
        .to_component_with_cast(descr.clone(), Selector::parse(".").unwrap(), CastTo::Auto)
        .build()
        .unwrap();

    let chunks = chunk.apply_lenses(&[lens], &runtime()).unwrap();

    insta::assert_snapshot!(DisplayRB(output_column(&chunks, descr.component)), @r"
    ┌─────────────────────────────────────────────────┐
    │ col                                             │
    │ ---                                             │
    │ type: List(FixedSizeList(3 x non-null Float32)) │
    ╞═════════════════════════════════════════════════╡
    │ [[1.0, 2.0, 3.0]]                               │
    └─────────────────────────────────────────────────┘
    ");
}

#[test]
fn cast_preserves_nulls() {
    // A null instance must survive the cast as a null, not a converted value.
    let descr = Scalars::descriptor_scalars();
    let chunk = chunk_with_value_column(vec![
        Arc::new(Int64Array::from(vec![Some(1)])) as ArrayRef,
        Arc::new(Int64Array::from(vec![None::<i64>])) as ArrayRef,
    ]);

    let lens = Lens::derive("value")
        .output_entity("out")
        .to_component_with_cast(descr.clone(), Selector::parse(".").unwrap(), CastTo::Auto)
        .build()
        .unwrap();

    let chunks = chunk.apply_lenses(&[lens], &runtime()).unwrap();

    insta::assert_snapshot!(DisplayRB(output_column(&chunks, descr.component)), @r"
    ┌─────────────────────┐
    │ col                 │
    │ ---                 │
    │ type: List(Float64) │
    ╞═════════════════════╡
    │ [1.0]               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [null]              │
    └─────────────────────┘
    ");
}

#[test]
fn scatter_path_casts_each_exploded_row() {
    // The cast runs before the explode, so the 1:N path is covered too.
    let descr = Scalars::descriptor_scalars();
    let chunk =
        chunk_with_value_column(vec![Arc::new(Int64Array::from(vec![1, 2, 3])) as ArrayRef]);

    let lens = Lens::scatter("value")
        .output_entity("out")
        .to_component_with_cast(descr.clone(), Selector::parse(".").unwrap(), CastTo::Auto)
        .build()
        .unwrap();

    let chunks = chunk.apply_lenses(&[lens], &runtime()).unwrap();

    insta::assert_snapshot!(DisplayRB(output_column(&chunks, descr.component)), @r"
    ┌─────────────────────┐
    │ col                 │
    │ ---                 │
    │ type: List(Float64) │
    ╞═════════════════════╡
    │ [1.0]               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [2.0]               │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ [3.0]               │
    └─────────────────────┘
    ");
}

#[test]
fn uncastable_type_errors() {
    // Utf8 -> FixedSizeList<_, 3> is not a supported Arrow cast.
    let chunk = chunk_with_value_column(vec![Arc::new(StringArray::from(vec!["hi"])) as ArrayRef]);
    let target =
        DataType::FixedSizeList(Arc::new(Field::new_list_field(DataType::Float64, true)), 3);

    let lens = Lens::derive("value")
        .output_entity("out")
        .to_component_with_cast(
            Scalars::descriptor_scalars(),
            Selector::parse(".").unwrap(),
            CastTo::Type(target),
        )
        .build()
        .unwrap();

    let err = chunk.apply_lenses(&[lens], &runtime()).unwrap_err();
    assert!(
        err.errors()
            .any(|e| matches!(e, LensRuntimeError::ComponentCastFailed { .. })),
        "expected an UncastableComponent error, got: {:?}",
        err.errors().collect::<Vec<_>>()
    );
}

#[test]
fn auto_cast_to_unregistered_component_errors() {
    // A `partial` descriptor carries no component type, so `Auto` has nothing to target.
    let chunk = chunk_with_value_column(vec![Arc::new(Int64Array::from(vec![1])) as ArrayRef]);

    let lens = Lens::derive("value")
        .output_entity("out")
        .to_component_with_cast(
            ComponentDescriptor::partial("custom"),
            Selector::parse(".").unwrap(),
            CastTo::Auto,
        )
        .build()
        .unwrap();

    let err = chunk.apply_lenses(&[lens], &runtime()).unwrap_err();
    assert!(
        err.errors()
            .any(|e| matches!(e, LensRuntimeError::UnknownComponentType { .. })),
        "expected an UnknownComponentType error, got: {:?}",
        err.errors().collect::<Vec<_>>()
    );
}
