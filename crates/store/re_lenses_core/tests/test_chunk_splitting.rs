#![expect(clippy::unwrap_used)]

use std::sync::Arc;

use arrow::array::{ArrayRef, Int32Array, Int32Builder, ListBuilder};
use re_chunk::{Chunk, ChunkId, TimeColumn, TimelineName};
use re_lenses_core::combinators::Error;
use re_lenses_core::{DynExpr, Lens, LensRuntimeError, Lenses, OutputMode, Selector};
use re_sdk_types::ComponentDescriptor;

fn example_selector() -> Selector<DynExpr> {
    fn times_42(source: &ArrayRef) -> Result<Option<ArrayRef>, Error> {
        let values = source
            .as_any()
            .downcast_ref::<Int32Array>()
            .ok_or_else(|| Error::TypeMismatch {
                expected: "Int32".into(),
                actual: source.data_type().clone(),
                context: "times_42".into(),
            })?;
        let result: Int32Array = values.iter().map(|v| v.map(|x| x * 42)).collect();
        Ok(Some(Arc::new(result)))
    }

    Selector::parse(".").unwrap().pipe(times_42)
}

/// Creates a chunk with three Int32 component columns (`alpha`, `beta`, `gamma`)
/// and a `tick` timeline with 2 rows.
fn three_component_chunk() -> Chunk {
    let make_column = |values: &[i32]| {
        let mut builder = ListBuilder::new(Int32Builder::new());
        for &v in values {
            builder.values().append_value(v);
            builder.append(true);
        }
        builder.finish()
    };

    let alpha = make_column(&[1, 2]);
    let beta = make_column(&[10, 20]);
    let gamma = make_column(&[100, 200]);

    let components = [
        (ComponentDescriptor::partial("alpha"), alpha),
        (ComponentDescriptor::partial("beta"), beta),
        (ComponentDescriptor::partial("gamma"), gamma),
    ]
    .into_iter();

    let time_column = TimeColumn::new_sequence("tick", 0..2);

    Chunk::from_auto_row_ids(
        ChunkId::new(),
        "test/entity".into(),
        std::iter::once((TimelineName::new("tick"), time_column)).collect(),
        components.collect(),
    )
    .unwrap()
}

#[test]
fn three_component_chunk_identity() {
    let chunk = three_component_chunk();
    insta::assert_snapshot!(format!("{:-240}", chunk), @r#"
    ┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                                                       │
    │ * entity_path: /test/entity                                                                                                     │
    │ * id: [**REDACTED**]                                                                                                            │
    │ * version: [**REDACTED**]                                                                                                       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌──────────────────────────────────────────────┬──────────────────┬───────────────────┬───────────────────┬───────────────────┐ │
    │ │ RowId                                        ┆ tick             ┆ alpha             ┆ beta              ┆ gamma             │ │
    │ │ ---                                          ┆ ---              ┆ ---               ┆ ---               ┆ ---               │ │
    │ │ type: non-null FixedSizeBinary(16)           ┆ type: Int64      ┆ type: List(Int32) ┆ type: List(Int32) ┆ type: List(Int32) │ │
    │ │ ARROW:extension:metadata:                    ┆ index_name: tick ┆ component: alpha  ┆ component: beta   ┆ component: gamma  │ │
    │ │ {"namespace":"row"}                          ┆ is_sorted: true  ┆ kind: data        ┆ kind: data        ┆ kind: data        │ │
    │ │ ARROW:extension:name: TUID                   ┆ kind: index      ┆                   ┆                   ┆                   │ │
    │ │ is_sorted: true                              ┆                  ┆                   ┆                   ┆                   │ │
    │ │ kind: control                                ┆                  ┆                   ┆                   ┆                   │ │
    │ ╞══════════════════════════════════════════════╪══════════════════╪═══════════════════╪═══════════════════╪═══════════════════╡ │
    │ │ row_[**REDACTED**]                           ┆ 0                ┆ [1]               ┆ [10]              ┆ [100]             │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                           ┆ 1                ┆ [2]               ┆ [20]              ┆ [200]             │ │
    │ └──────────────────────────────────────────────┴──────────────────┴───────────────────┴───────────────────┴───────────────────┘ │
    └─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
    "#);
}

/// Two lenses consume `alpha` and `beta`; `gamma` is untouched.
///
/// With [`OutputMode::ForwardUnmatched`], same-entity outputs are merged into
/// a single chunk alongside the forwarded `gamma`.
#[test]
fn forward_unmatched_merges_same_entity_outputs() {
    let chunk = three_component_chunk();
    let original_row_ids = chunk.row_ids_slice();

    let lens_alpha = Lens::derive("alpha")
        .to_component(
            ComponentDescriptor::partial("alpha_out"),
            example_selector(),
        )
        .build()
        .unwrap();

    let lens_beta = Lens::derive("beta")
        .to_component(ComponentDescriptor::partial("beta_out"), example_selector())
        .build()
        .unwrap();

    let lenses = Lenses::new(OutputMode::ForwardUnmatched)
        .add_lens(lens_alpha)
        .add_lens(lens_beta);

    let results: Vec<_> = lenses.apply(&chunk).collect::<Result<_, _>>().unwrap();
    assert_eq!(results.len(), 1);
    assert_ne!(results[0].id(), chunk.id());
    assert_eq!(results[0].row_ids_slice(), original_row_ids);
    insta::assert_snapshot!(format!("{:-240}", results[0]), @r#"
    ┌──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                                                            │
    │ * entity_path: /test/entity                                                                                                          │
    │ * id: [**REDACTED**]                                                                                                                 │
    │ * version: [**REDACTED**]                                                                                                            │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌──────────────────────────────────────────────┬──────────────────┬──────────────────────┬─────────────────────┬───────────────────┐ │
    │ │ RowId                                        ┆ tick             ┆ alpha_out            ┆ beta_out            ┆ gamma             │ │
    │ │ ---                                          ┆ ---              ┆ ---                  ┆ ---                 ┆ ---               │ │
    │ │ type: non-null FixedSizeBinary(16)           ┆ type: Int64      ┆ type: List(Int32)    ┆ type: List(Int32)   ┆ type: List(Int32) │ │
    │ │ ARROW:extension:metadata:                    ┆ index_name: tick ┆ component: alpha_out ┆ component: beta_out ┆ component: gamma  │ │
    │ │ {"namespace":"row"}                          ┆ is_sorted: true  ┆ kind: data           ┆ kind: data          ┆ kind: data        │ │
    │ │ ARROW:extension:name: TUID                   ┆ kind: index      ┆                      ┆                     ┆                   │ │
    │ │ is_sorted: true                              ┆                  ┆                      ┆                     ┆                   │ │
    │ │ kind: control                                ┆                  ┆                      ┆                     ┆                   │ │
    │ ╞══════════════════════════════════════════════╪══════════════════╪══════════════════════╪═════════════════════╪═══════════════════╡ │
    │ │ row_[**REDACTED**]                           ┆ 0                ┆ [42]                 ┆ [420]               ┆ [100]             │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                           ┆ 1                ┆ [84]                 ┆ [840]               ┆ [200]             │ │
    │ └──────────────────────────────────────────────┴──────────────────┴──────────────────────┴─────────────────────┴───────────────────┘ │
    └──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
    "#);
}

/// Three lenses consume all three components; nothing is untouched.
///
/// With [`OutputMode::ForwardUnmatched`], no prefix chunk should be emitted since
/// every component is consumed by a lens.
#[test]
fn forward_unmatched_no_prefix_when_all_consumed() {
    let chunk = three_component_chunk();
    let original_row_ids = chunk.row_ids_slice();

    let make_lens = |input: &str, output: &str| {
        Lens::derive(input)
            .output_entity(input)
            .to_component(
                ComponentDescriptor::partial(output),
                Selector::parse(".").unwrap(),
            )
            .build()
            .unwrap()
    };

    let lenses = Lenses::new(OutputMode::ForwardUnmatched)
        .add_lens(make_lens("alpha", "alpha_out"))
        .add_lens(make_lens("beta", "beta_out"))
        .add_lens(make_lens("gamma", "gamma_out"));

    let results: Vec<_> = lenses.apply(&chunk).collect::<Result<_, _>>().unwrap();
    assert_eq!(results.len(), 3);
    for result in &results {
        assert_ne!(result.id(), chunk.id());
        assert_ne!(result.row_ids_slice(), original_row_ids);
    }

    insta::assert_snapshot!(format!("{:-240}", results[0]), @r#"
    ┌─────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                   │
    │ * entity_path: /alpha                                                                       │
    │ * id: [**REDACTED**]                                                                        │
    │ * version: [**REDACTED**]                                                                   │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌───────────────────────────────────────────────┬──────────────────┬──────────────────────┐ │
    │ │ RowId                                         ┆ tick             ┆ alpha_out            │ │
    │ │ ---                                           ┆ ---              ┆ ---                  │ │
    │ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64      ┆ type: List(Int32)    │ │
    │ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: tick ┆ component: alpha_out │ │
    │ │ ARROW:extension:name: TUID                    ┆ is_sorted: true  ┆ kind: data           │ │
    │ │ is_sorted: true                               ┆ kind: index      ┆                      │ │
    │ │ kind: control                                 ┆                  ┆                      │ │
    │ ╞═══════════════════════════════════════════════╪══════════════════╪══════════════════════╡ │
    │ │ row_[**REDACTED**]                            ┆ 0                ┆ [1]                  │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                            ┆ 1                ┆ [2]                  │ │
    │ └───────────────────────────────────────────────┴──────────────────┴──────────────────────┘ │
    └─────────────────────────────────────────────────────────────────────────────────────────────┘
    "#);
    insta::assert_snapshot!(format!("{:-240}", results[1]), @r#"
    ┌────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                  │
    │ * entity_path: /beta                                                                       │
    │ * id: [**REDACTED**]                                                                       │
    │ * version: [**REDACTED**]                                                                  │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌───────────────────────────────────────────────┬──────────────────┬─────────────────────┐ │
    │ │ RowId                                         ┆ tick             ┆ beta_out            │ │
    │ │ ---                                           ┆ ---              ┆ ---                 │ │
    │ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64      ┆ type: List(Int32)   │ │
    │ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: tick ┆ component: beta_out │ │
    │ │ ARROW:extension:name: TUID                    ┆ is_sorted: true  ┆ kind: data          │ │
    │ │ is_sorted: true                               ┆ kind: index      ┆                     │ │
    │ │ kind: control                                 ┆                  ┆                     │ │
    │ ╞═══════════════════════════════════════════════╪══════════════════╪═════════════════════╡ │
    │ │ row_[**REDACTED**]                            ┆ 0                ┆ [10]                │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                            ┆ 1                ┆ [20]                │ │
    │ └───────────────────────────────────────────────┴──────────────────┴─────────────────────┘ │
    └────────────────────────────────────────────────────────────────────────────────────────────┘
    "#);
    insta::assert_snapshot!(format!("{:-240}", results[2]), @r#"
    ┌─────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                   │
    │ * entity_path: /gamma                                                                       │
    │ * id: [**REDACTED**]                                                                        │
    │ * version: [**REDACTED**]                                                                   │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌───────────────────────────────────────────────┬──────────────────┬──────────────────────┐ │
    │ │ RowId                                         ┆ tick             ┆ gamma_out            │ │
    │ │ ---                                           ┆ ---              ┆ ---                  │ │
    │ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64      ┆ type: List(Int32)    │ │
    │ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: tick ┆ component: gamma_out │ │
    │ │ ARROW:extension:name: TUID                    ┆ is_sorted: true  ┆ kind: data           │ │
    │ │ is_sorted: true                               ┆ kind: index      ┆                      │ │
    │ │ kind: control                                 ┆                  ┆                      │ │
    │ ╞═══════════════════════════════════════════════╪══════════════════╪══════════════════════╡ │
    │ │ row_[**REDACTED**]                            ┆ 0                ┆ [100]                │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                            ┆ 1                ┆ [200]                │ │
    │ └───────────────────────────────────────────────┴──────────────────┴──────────────────────┘ │
    └─────────────────────────────────────────────────────────────────────────────────────────────┘
    "#);
}

/// A mutate-only lens modifies the prefix without producing a separate output chunk.
#[test]
fn mutate_only_modifies_prefix() {
    let chunk = three_component_chunk();
    let original_row_ids = chunk.row_ids_slice();

    let lens = Lens::mutate("alpha", example_selector()).build();

    let lenses = Lenses::new(OutputMode::ForwardUnmatched).add_lens(lens);
    let results: Vec<_> = lenses.apply(&chunk).collect::<Result<_, _>>().unwrap();

    // Single chunk: the prefix with alpha modified in-place + beta + gamma.
    assert_eq!(results.len(), 1);
    assert_ne!(results[0].id(), chunk.id());
    assert_ne!(results[0].row_ids_slice(), original_row_ids);
    insta::assert_snapshot!(format!("{:-240}", results[0]), @r#"
    ┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                                                       │
    │ * entity_path: /test/entity                                                                                                     │
    │ * id: [**REDACTED**]                                                                                                            │
    │ * version: [**REDACTED**]                                                                                                       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌──────────────────────────────────────────────┬──────────────────┬───────────────────┬───────────────────┬───────────────────┐ │
    │ │ RowId                                        ┆ tick             ┆ alpha             ┆ beta              ┆ gamma             │ │
    │ │ ---                                          ┆ ---              ┆ ---               ┆ ---               ┆ ---               │ │
    │ │ type: non-null FixedSizeBinary(16)           ┆ type: Int64      ┆ type: List(Int32) ┆ type: List(Int32) ┆ type: List(Int32) │ │
    │ │ ARROW:extension:metadata:                    ┆ index_name: tick ┆ component: alpha  ┆ component: beta   ┆ component: gamma  │ │
    │ │ {"namespace":"row"}                          ┆ is_sorted: true  ┆ kind: data        ┆ kind: data        ┆ kind: data        │ │
    │ │ ARROW:extension:name: TUID                   ┆ kind: index      ┆                   ┆                   ┆                   │ │
    │ │ is_sorted: true                              ┆                  ┆                   ┆                   ┆                   │ │
    │ │ kind: control                                ┆                  ┆                   ┆                   ┆                   │ │
    │ ╞══════════════════════════════════════════════╪══════════════════╪═══════════════════╪═══════════════════╪═══════════════════╡ │
    │ │ row_[**REDACTED**]                           ┆ 0                ┆ [42]              ┆ [10]              ┆ [100]             │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                           ┆ 1                ┆ [84]              ┆ [20]              ┆ [200]             │ │
    │ └──────────────────────────────────────────────┴──────────────────┴───────────────────┴───────────────────┴───────────────────┘ │
    └─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
    "#);
}

/// `mutate_keep_row_ids` preserves the original chunk's `RowIds`.
#[test]
fn mutate_keep_row_ids_preserves_row_ids() {
    let chunk = three_component_chunk();
    let original_row_ids = chunk.row_ids_slice();

    let lens = Lens::mutate("alpha", example_selector())
        .keep_row_ids()
        .build();

    let lenses = Lenses::new(OutputMode::ForwardUnmatched).add_lens(lens);
    let results: Vec<_> = lenses.apply(&chunk).collect::<Result<_, _>>().unwrap();

    assert_eq!(results.len(), 1);
    assert_ne!(results[0].id(), chunk.id());
    assert_eq!(results[0].row_ids_slice(), original_row_ids);
    insta::assert_snapshot!(format!("{:-240}", results[0]), @r#"
    ┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                                                       │
    │ * entity_path: /test/entity                                                                                                     │
    │ * id: [**REDACTED**]                                                                                                            │
    │ * version: [**REDACTED**]                                                                                                       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌──────────────────────────────────────────────┬──────────────────┬───────────────────┬───────────────────┬───────────────────┐ │
    │ │ RowId                                        ┆ tick             ┆ alpha             ┆ beta              ┆ gamma             │ │
    │ │ ---                                          ┆ ---              ┆ ---               ┆ ---               ┆ ---               │ │
    │ │ type: non-null FixedSizeBinary(16)           ┆ type: Int64      ┆ type: List(Int32) ┆ type: List(Int32) ┆ type: List(Int32) │ │
    │ │ ARROW:extension:metadata:                    ┆ index_name: tick ┆ component: alpha  ┆ component: beta   ┆ component: gamma  │ │
    │ │ {"namespace":"row"}                          ┆ is_sorted: true  ┆ kind: data        ┆ kind: data        ┆ kind: data        │ │
    │ │ ARROW:extension:name: TUID                   ┆ kind: index      ┆                   ┆                   ┆                   │ │
    │ │ is_sorted: true                              ┆                  ┆                   ┆                   ┆                   │ │
    │ │ kind: control                                ┆                  ┆                   ┆                   ┆                   │ │
    │ ╞══════════════════════════════════════════════╪══════════════════╪═══════════════════╪═══════════════════╪═══════════════════╡ │
    │ │ row_[**REDACTED**]                           ┆ 0                ┆ [42]              ┆ [10]              ┆ [100]             │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                           ┆ 1                ┆ [84]              ┆ [20]              ┆ [200]             │ │
    │ └──────────────────────────────────────────────┴──────────────────┴───────────────────┴───────────────────┴───────────────────┘ │
    └─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
    "#);
}

/// `mutate` without `keep_row_ids` generates new `RowIds`.
#[test]
fn mutate_without_keep_generates_new_row_ids() {
    let chunk = three_component_chunk();
    let original_row_ids = chunk.row_ids_slice();

    let lens = Lens::mutate("alpha", example_selector()).build();

    let lenses = Lenses::new(OutputMode::ForwardUnmatched).add_lens(lens);
    let results: Vec<_> = lenses.apply(&chunk).collect::<Result<_, _>>().unwrap();

    assert_eq!(results.len(), 1);
    assert_ne!(results[0].id(), chunk.id());
    assert_ne!(results[0].row_ids_slice(), original_row_ids);
    insta::assert_snapshot!(format!("{:-240}", results[0]), @r#"
    ┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                                                       │
    │ * entity_path: /test/entity                                                                                                     │
    │ * id: [**REDACTED**]                                                                                                            │
    │ * version: [**REDACTED**]                                                                                                       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌──────────────────────────────────────────────┬──────────────────┬───────────────────┬───────────────────┬───────────────────┐ │
    │ │ RowId                                        ┆ tick             ┆ alpha             ┆ beta              ┆ gamma             │ │
    │ │ ---                                          ┆ ---              ┆ ---               ┆ ---               ┆ ---               │ │
    │ │ type: non-null FixedSizeBinary(16)           ┆ type: Int64      ┆ type: List(Int32) ┆ type: List(Int32) ┆ type: List(Int32) │ │
    │ │ ARROW:extension:metadata:                    ┆ index_name: tick ┆ component: alpha  ┆ component: beta   ┆ component: gamma  │ │
    │ │ {"namespace":"row"}                          ┆ is_sorted: true  ┆ kind: data        ┆ kind: data        ┆ kind: data        │ │
    │ │ ARROW:extension:name: TUID                   ┆ kind: index      ┆                   ┆                   ┆                   │ │
    │ │ is_sorted: true                              ┆                  ┆                   ┆                   ┆                   │ │
    │ │ kind: control                                ┆                  ┆                   ┆                   ┆                   │ │
    │ ╞══════════════════════════════════════════════╪══════════════════╪═══════════════════╪═══════════════════╪═══════════════════╡ │
    │ │ row_[**REDACTED**]                           ┆ 0                ┆ [42]              ┆ [10]              ┆ [100]             │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                           ┆ 1                ┆ [84]              ┆ [20]              ┆ [200]             │ │
    │ └──────────────────────────────────────────────┴──────────────────┴───────────────────┴───────────────────┴───────────────────┘ │
    └─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
    "#);
}

/// When two same-entity lenses claim the same output component, the first wins
/// and the second is skipped with a `DeriveCollision` error.
#[test]
fn same_entity_collision_skips_duplicate() {
    let chunk = three_component_chunk();
    let original_row_ids = chunk.row_ids_slice();

    let lens_a = Lens::derive("alpha")
        .to_component(ComponentDescriptor::partial("shared"), example_selector())
        .build()
        .unwrap();

    let lens_b = Lens::derive("beta")
        .to_component(ComponentDescriptor::partial("shared"), example_selector())
        .build()
        .unwrap();

    let lenses = Lenses::new(OutputMode::ForwardUnmatched)
        .add_lens(lens_a)
        .add_lens(lens_b);
    let mut results: Vec<_> = lenses.apply(&chunk).collect();

    // The prefix carries a DeriveCollision error alongside the partial chunk.
    assert_eq!(results.len(), 1);
    let err = results.remove(0).unwrap_err();
    {
        let errors: Vec<_> = err.errors().collect();
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(errors[0], LensRuntimeError::DeriveCollision { .. }),
            "expected DeriveCollision, got: {errors:?}",
        );
    }

    // Partial chunk: gamma (forwarded) + shared (from lens_a, *42).
    let partial = err.partial_chunk().unwrap();
    assert_ne!(partial.id(), chunk.id());
    assert_eq!(partial.row_ids_slice(), original_row_ids);
    insta::assert_snapshot!(format!("{:-240}", partial), @r#"
    ┌──────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                                    │
    │ * entity_path: /test/entity                                                                                  │
    │ * id: [**REDACTED**]                                                                                         │
    │ * version: [**REDACTED**]                                                                                    │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌───────────────────────────────────────────────┬──────────────────┬───────────────────┬───────────────────┐ │
    │ │ RowId                                         ┆ tick             ┆ gamma             ┆ shared            │ │
    │ │ ---                                           ┆ ---              ┆ ---               ┆ ---               │ │
    │ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64      ┆ type: List(Int32) ┆ type: List(Int32) │ │
    │ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: tick ┆ component: gamma  ┆ component: shared │ │
    │ │ ARROW:extension:name: TUID                    ┆ is_sorted: true  ┆ kind: data        ┆ kind: data        │ │
    │ │ is_sorted: true                               ┆ kind: index      ┆                   ┆                   │ │
    │ │ kind: control                                 ┆                  ┆                   ┆                   │ │
    │ ╞═══════════════════════════════════════════════╪══════════════════╪═══════════════════╪═══════════════════╡ │
    │ │ row_[**REDACTED**]                            ┆ 0                ┆ [100]             ┆ [42]              │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                            ┆ 1                ┆ [200]             ┆ [84]              │ │
    │ └───────────────────────────────────────────────┴──────────────────┴───────────────────┴───────────────────┘ │
    └──────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
    "#);
}

/// When two new-entity lenses targeting the same entity claim the same output
/// component, the first wins and the second is skipped with a `DeriveCollision` error.
#[test]
fn new_entity_collision_skips_duplicate() {
    let chunk = three_component_chunk();

    let lens_a = Lens::derive("alpha")
        .output_entity("new_entity")
        .to_component(ComponentDescriptor::partial("shared"), example_selector())
        .build()
        .unwrap();

    let lens_b = Lens::derive("beta")
        .output_entity("new_entity")
        .to_component(ComponentDescriptor::partial("shared"), example_selector())
        .build()
        .unwrap();

    let lenses = Lenses::new(OutputMode::DropUnmatched)
        .add_lens(lens_a)
        .add_lens(lens_b);
    let mut results: Vec<_> = lenses.apply(&chunk).collect();

    // Two items: first is the error-carrying prefix (no forwarded columns with
    // DropUnmatched), second is the successful derive chunk from lens_a.
    assert_eq!(results.len(), 2);

    // First result: error with no partial chunk (DropUnmatched, no modifications).
    let err = results.remove(0).unwrap_err();
    {
        let errors: Vec<_> = err.errors().collect();
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(errors[0], LensRuntimeError::DeriveCollision { .. }),
            "expected DeriveCollision, got: {errors:?}",
        );
    }
    assert!(err.partial_chunk().is_none());

    // Second result: successful derive chunk from lens_a only.
    let derived = results.remove(0).unwrap();
    assert_ne!(derived.id(), chunk.id());
    assert_ne!(derived.row_ids_slice(), chunk.row_ids_slice());
    assert_eq!(derived.entity_path(), &"new_entity".into());
    insta::assert_snapshot!(format!("{:-240}", derived), @r#"
    ┌──────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                │
    │ * entity_path: /new_entity                                                               │
    │ * id: [**REDACTED**]                                                                     │
    │ * version: [**REDACTED**]                                                                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌───────────────────────────────────────────────┬──────────────────┬───────────────────┐ │
    │ │ RowId                                         ┆ tick             ┆ shared            │ │
    │ │ ---                                           ┆ ---              ┆ ---               │ │
    │ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64      ┆ type: List(Int32) │ │
    │ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: tick ┆ component: shared │ │
    │ │ ARROW:extension:name: TUID                    ┆ is_sorted: true  ┆ kind: data        │ │
    │ │ is_sorted: true                               ┆ kind: index      ┆                   │ │
    │ │ kind: control                                 ┆                  ┆                   │ │
    │ ╞═══════════════════════════════════════════════╪══════════════════╪═══════════════════╡ │
    │ │ row_[**REDACTED**]                            ┆ 0                ┆ [42]              │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                            ┆ 1                ┆ [84]              │ │
    │ └───────────────────────────────────────────────┴──────────────────┴───────────────────┘ │
    └──────────────────────────────────────────────────────────────────────────────────────────┘
    "#);
}

/// Two mutate lenses targeting the same component produce a `MutateCollision` error
/// with a partial chunk containing the first mutation applied.
#[test]
fn mutate_collision_returns_error() {
    let chunk = three_component_chunk();

    let lenses = Lenses::new(OutputMode::ForwardUnmatched)
        .add_lens(Lens::mutate("alpha", example_selector()).build())
        .add_lens(Lens::mutate("alpha", example_selector()).build());

    let mut results: Vec<_> = lenses.apply(&chunk).collect();

    // The prefix chunk is an Err carrying the collision.
    assert_eq!(results.len(), 1);
    let err = results.remove(0).unwrap_err();

    {
        let errors: Vec<_> = err.errors().collect();
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(errors[0], LensRuntimeError::MutateCollision { .. }),
            "expected MutateCollision, got: {errors:?}",
        );
    }

    // The partial chunk still contains the first mutation applied.
    let partial = err.partial_chunk().unwrap();
    insta::assert_snapshot!(format!("{:-240}", partial), @r#"
    ┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                                                       │
    │ * entity_path: /test/entity                                                                                                     │
    │ * id: [**REDACTED**]                                                                                                            │
    │ * version: [**REDACTED**]                                                                                                       │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌──────────────────────────────────────────────┬──────────────────┬───────────────────┬───────────────────┬───────────────────┐ │
    │ │ RowId                                        ┆ tick             ┆ alpha             ┆ beta              ┆ gamma             │ │
    │ │ ---                                          ┆ ---              ┆ ---               ┆ ---               ┆ ---               │ │
    │ │ type: non-null FixedSizeBinary(16)           ┆ type: Int64      ┆ type: List(Int32) ┆ type: List(Int32) ┆ type: List(Int32) │ │
    │ │ ARROW:extension:metadata:                    ┆ index_name: tick ┆ component: alpha  ┆ component: beta   ┆ component: gamma  │ │
    │ │ {"namespace":"row"}                          ┆ is_sorted: true  ┆ kind: data        ┆ kind: data        ┆ kind: data        │ │
    │ │ ARROW:extension:name: TUID                   ┆ kind: index      ┆                   ┆                   ┆                   │ │
    │ │ is_sorted: true                              ┆                  ┆                   ┆                   ┆                   │ │
    │ │ kind: control                                ┆                  ┆                   ┆                   ┆                   │ │
    │ ╞══════════════════════════════════════════════╪══════════════════╪═══════════════════╪═══════════════════╪═══════════════════╡ │
    │ │ row_[**REDACTED**]                           ┆ 0                ┆ [42]              ┆ [10]              ┆ [100]             │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                           ┆ 1                ┆ [84]              ┆ [20]              ┆ [200]             │ │
    │ └──────────────────────────────────────────────┴──────────────────┴───────────────────┴───────────────────┴───────────────────┘ │
    └─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
    "#);
}

/// A derive lens and a mutate lens both target the same input component.
#[test]
fn derive_and_mutate_on_same_input() {
    let chunk = three_component_chunk();
    let original_row_ids = chunk.row_ids_slice();

    let mutate = Lens::mutate("alpha", example_selector()).build();
    let derive = Lens::derive("alpha")
        .to_component(
            ComponentDescriptor::partial("alpha_out"),
            example_selector(),
        )
        .build()
        .unwrap();

    let lenses = Lenses::new(OutputMode::ForwardUnmatched)
        .add_lens(mutate)
        .add_lens(derive);
    let results: Vec<_> = lenses.apply(&chunk).collect::<Result<_, _>>().unwrap();

    // Single merged chunk: alpha modified in-place (*42), alpha_out derived (*42),
    // beta and gamma forwarded unchanged.
    assert_eq!(results.len(), 1);
    assert_ne!(results[0].id(), chunk.id());
    assert_ne!(results[0].row_ids_slice(), original_row_ids);
    insta::assert_snapshot!(format!("{:-240}", results[0]), @r#"
    ┌───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                                                                     │
    │ * entity_path: /test/entity                                                                                                                   │
    │ * id: [**REDACTED**]                                                                                                                          │
    │ * version: [**REDACTED**]                                                                                                                     │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌─────────────────────────────────────┬──────────────────┬───────────────────┬──────────────────────┬───────────────────┬───────────────────┐ │
    │ │ RowId                               ┆ tick             ┆ alpha             ┆ alpha_out            ┆ beta              ┆ gamma             │ │
    │ │ ---                                 ┆ ---              ┆ ---               ┆ ---                  ┆ ---               ┆ ---               │ │
    │ │ type: non-null FixedSizeBinary(16)  ┆ type: Int64      ┆ type: List(Int32) ┆ type: List(Int32)    ┆ type: List(Int32) ┆ type: List(Int32) │ │
    │ │ ARROW:extension:metadata:           ┆ index_name: tick ┆ component: alpha  ┆ component: alpha_out ┆ component: beta   ┆ component: gamma  │ │
    │ │ {"namespace":"row"}                 ┆ is_sorted: true  ┆ kind: data        ┆ kind: data           ┆ kind: data        ┆ kind: data        │ │
    │ │ ARROW:extension:name: TUID          ┆ kind: index      ┆                   ┆                      ┆                   ┆                   │ │
    │ │ is_sorted: true                     ┆                  ┆                   ┆                      ┆                   ┆                   │ │
    │ │ kind: control                       ┆                  ┆                   ┆                      ┆                   ┆                   │ │
    │ ╞═════════════════════════════════════╪══════════════════╪═══════════════════╪══════════════════════╪═══════════════════╪═══════════════════╡ │
    │ │ row_[**REDACTED**]                  ┆ 0                ┆ [42]              ┆ [42]                 ┆ [10]              ┆ [100]             │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                  ┆ 1                ┆ [84]              ┆ [84]                 ┆ [20]              ┆ [200]             │ │
    │ └─────────────────────────────────────┴──────────────────┴───────────────────┴──────────────────────┴───────────────────┴───────────────────┘ │
    └───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
    "#);
}
