#![expect(clippy::unwrap_used)]

use arrow::array::{Int32Builder, ListBuilder};
use re_chunk::{Chunk, ChunkId, TimeColumn, TimelineName};
use re_lenses_core::{Lens, Lenses, OutputMode, Selector};
use re_sdk_types::ComponentDescriptor;

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
/// With [`OutputMode::ForwardUnmatched`], the result should be three separate chunks
/// with no duplicate component columns:
/// 1. A prefix chunk containing only `gamma`
/// 2. Lens output containing `alpha_out`
/// 3. Lens output containing `beta_out`
#[test]
fn forward_unmatched_splits_components_across_chunks() {
    let chunk = three_component_chunk();

    let lens_alpha = Lens::for_input_column("alpha")
        .output_columns(|out| {
            out.component(
                ComponentDescriptor::partial("alpha_out"),
                Selector::parse(".")?,
            )
        })
        .unwrap()
        .build();

    let lens_beta = Lens::for_input_column("beta")
        .output_columns(|out| {
            out.component(
                ComponentDescriptor::partial("beta_out"),
                Selector::parse(".")?,
            )
        })
        .unwrap()
        .build();

    let lenses = Lenses::new(OutputMode::ForwardUnmatched)
        .add_lens(lens_alpha)
        .add_lens(lens_beta);

    let results: Vec<_> = lenses.apply(&chunk).collect::<Result<_, _>>().unwrap();
    assert_eq!(results.len(), 3);

    // Chunk 0: prefix with only the untouched component.
    insta::assert_snapshot!(format!("{:-240}", results[0]), @r#"
    ┌──────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                │
    │ * entity_path: /test/entity                                                              │
    │ * id: [**REDACTED**]                                                                     │
    │ * version: [**REDACTED**]                                                                │
    ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
    │ ┌───────────────────────────────────────────────┬──────────────────┬───────────────────┐ │
    │ │ RowId                                         ┆ tick             ┆ gamma             │ │
    │ │ ---                                           ┆ ---              ┆ ---               │ │
    │ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64      ┆ type: List(Int32) │ │
    │ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: tick ┆ component: gamma  │ │
    │ │ ARROW:extension:name: TUID                    ┆ is_sorted: true  ┆ kind: data        │ │
    │ │ is_sorted: true                               ┆ kind: index      ┆                   │ │
    │ │ kind: control                                 ┆                  ┆                   │ │
    │ ╞═══════════════════════════════════════════════╪══════════════════╪═══════════════════╡ │
    │ │ row_[**REDACTED**]                            ┆ 0                ┆ [100]             │ │
    │ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
    │ │ row_[**REDACTED**]                            ┆ 1                ┆ [200]             │ │
    │ └───────────────────────────────────────────────┴──────────────────┴───────────────────┘ │
    └──────────────────────────────────────────────────────────────────────────────────────────┘
    "#);

    // Chunk 1: lens output for alpha.
    insta::assert_snapshot!(format!("{:-240}", results[1]), @r#"
    ┌─────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                   │
    │ * entity_path: /test/entity                                                                 │
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

    // Chunk 2: lens output for beta.
    insta::assert_snapshot!(format!("{:-240}", results[2]), @r#"
    ┌────────────────────────────────────────────────────────────────────────────────────────────┐
    │ METADATA:                                                                                  │
    │ * entity_path: /test/entity                                                                │
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
}

/// Three lenses consume all three components; nothing is untouched.
///
/// With [`OutputMode::ForwardUnmatched`], no prefix chunk should be emitted since
/// every component is consumed by a lens.
#[test]
fn forward_unmatched_no_prefix_when_all_consumed() {
    let chunk = three_component_chunk();

    let make_lens = |input: &str, output: &str| {
        Lens::for_input_column(input)
            .output_columns_at(input, |out| {
                out.component(ComponentDescriptor::partial(output), Selector::parse(".")?)
            })
            .unwrap()
            .build()
    };

    let lenses = Lenses::new(OutputMode::ForwardUnmatched)
        .add_lens(make_lens("alpha", "alpha_out"))
        .add_lens(make_lens("beta", "beta_out"))
        .add_lens(make_lens("gamma", "gamma_out"));

    let results: Vec<_> = lenses.apply(&chunk).collect::<Result<_, _>>().unwrap();
    assert_eq!(results.len(), 3);

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
