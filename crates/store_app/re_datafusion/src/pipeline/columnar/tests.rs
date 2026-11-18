use arrow::compute::concat_batches;
use re_dataframe::ViewContentsSelector;
use re_dataframe::external::re_chunk_store::ChunkStoreConfig;
use re_log_types::{StoreId, StoreKind, TimePoint, TimeType, Timeline};
use re_sorbet::ComponentColumnSelector;
use re_types_core::ComponentDescriptor;

use super::super::RowEmitterKind;
use super::super::emitter::SegmentRowEmitter;
use super::super::reference::{self, ReferenceQuery};
use super::super::test_support::{
    FetchBehavior, TEST_ENTITY, chunk_info_batch_sized, clear_chunk, column_header,
    emitted_component_values, emitted_segment_ids, recursive_clear_chunk,
    run_driver_projected_with, scalar_chunk, scalar_chunk_on_component, scalar_rows_chunk,
    schema_with_component, test_component,
};
use super::*;
use crate::dataframe_query_common::prepend_string_column_schema;

const INDEX: &str = "frame";
const OTHER_INDEX: &str = "other";

fn index() -> TimelineName {
    TimelineName::try_new(INDEX).unwrap()
}

fn query(
    view: &[(&str, &[ComponentDescriptor])],
    not_null: Option<(&str, &ComponentDescriptor)>,
) -> QueryExpression {
    let mut contents: BTreeMap<EntityPath, Option<BTreeSet<_>>> = BTreeMap::new();
    for (entity, descrs) in view {
        contents.insert(
            EntityPath::from(*entity),
            Some(descrs.iter().map(|d| d.component).collect()),
        );
    }
    QueryExpression {
        view_contents: Some(ViewContentsSelector(contents)),
        filtered_index: Some(index()),
        filtered_is_not_null: not_null.map(|(entity, descr)| ComponentColumnSelector {
            entity_path: EntityPath::from(entity),
            component: descr.component.to_string(),
        }),
        ..Default::default()
    }
}

/// A chunk with `(index, other, value)` rows on [`INDEX`], of type
/// `index_type`, and on [`OTHER_INDEX`], of type `other_type`.
fn two_timeline_chunk(
    entity: &str,
    index_type: TimeType,
    other_type: TimeType,
    rows: &[(i64, i64, i64)],
) -> Chunk {
    let mut builder = Chunk::builder(EntityPath::from(entity));
    for &(t, other, value) in rows {
        let timepoint = TimePoint::from([
            (Timeline::new(INDEX, index_type), TimeInt::new_temporal(t)),
            (
                Timeline::new(OTHER_INDEX, other_type),
                TimeInt::new_temporal(other),
            ),
        ]);
        let array: ArrayRef = Arc::new(Int64Array::from(vec![value]));
        builder = builder.with_row(RowId::new(), timepoint, [(test_component(), array)]);
    }
    builder.build().unwrap()
}

fn store_of(chunks: &[Arc<Chunk>]) -> ChunkStore {
    let mut store = ChunkStore::new(
        StoreId::random(StoreKind::Recording, "columnar_test"),
        ChunkStoreConfig::ALL_DISABLED,
    );
    for chunk in chunks {
        store.insert_chunk(chunk).unwrap();
    }
    store
}

/// The schema v2 projects for `query` over `chunks`.
fn projected_schema(chunks: &[Arc<Chunk>], query: &QueryExpression) -> Arc<Schema> {
    let fields: Vec<Field> = store_of(chunks)
        .schema_for_query(query)
        .indices_and_components()
        .iter()
        .map(|descr| descr.to_arrow_field(BatchType::Dataframe))
        .collect();
    Arc::new(prepend_string_column_schema(
        &Schema::new_with_metadata(fields, Default::default()),
        ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
    ))
}

/// The rows an emitter of `kind` gives: one `flush_incremental_to` per
/// horizon, then the final `flush`. Returns the rows and the chunks GC
/// removed per horizon.
async fn emit(
    kind: RowEmitterKind,
    chunks: &[Arc<Chunk>],
    query: &QueryExpression,
    projected: &Arc<Schema>,
    horizons: &[i64],
    limit_rows: Option<usize>,
) -> (RecordBatch, Vec<usize>) {
    let arrivals = vec![0; chunks.len()];
    emit_arriving(
        kind, chunks, &arrivals, query, projected, horizons, limit_rows,
    )
    .await
}

/// [`emit`] with `chunks[i]` inserted just before flush `arrivals[i]`
/// (`horizons.len()` being the final `flush`), as the driver delivers them.
async fn emit_arriving(
    kind: RowEmitterKind,
    chunks: &[Arc<Chunk>],
    arrivals: &[usize],
    query: &QueryExpression,
    projected: &Arc<Schema>,
    horizons: &[i64],
    limit_rows: Option<usize>,
) -> (RecordBatch, Vec<usize>) {
    let mut emitter =
        SegmentRowEmitter::new(re_uri::Origin::test(), "seg-a".into(), query, &None, kind);
    assert_eq!(emitter.kind(), kind);
    let insert_arrivals = |emitter: &mut SegmentRowEmitter, step: usize| {
        for (chunk, _) in std::iter::zip(chunks, arrivals).filter(|(_, a)| **a == step) {
            emitter.insert_chunk(chunk).unwrap();
        }
    };
    let (tx, mut rx) = tokio::sync::mpsc::channel::<RecordBatch>(1024);
    let mut rows_sent = 0;
    let mut removed = Vec::new();
    for (step, &horizon) in horizons.iter().enumerate() {
        insert_arrivals(&mut emitter, step);
        let outcome = emitter
            .flush_incremental_to(
                TimeInt::new_temporal(horizon),
                projected,
                &tx,
                &mut rows_sent,
                limit_rows,
            )
            .await
            .unwrap();
        removed.push(outcome.gc.removed_chunks.len());
    }
    insert_arrivals(&mut emitter, horizons.len());
    emitter
        .flush(projected, &tx, &mut rows_sent, limit_rows)
        .await
        .unwrap();
    drop(tx);
    let mut batches = Vec::new();
    while let Some(batch) = rx.recv().await {
        batches.push(batch);
    }
    (concat_batches(projected, &batches).unwrap(), removed)
}

fn assert_same_rows(actual: &RecordBatch, expected: &RecordBatch) {
    assert_eq!(actual.num_rows(), expected.num_rows(), "row count");
    for (i, field) in expected.schema().fields().iter().enumerate() {
        assert_eq!(
            actual.column(i),
            expected.column(i),
            "column {}",
            field.name()
        );
    }
}

fn arcs(chunks: impl IntoIterator<Item = Chunk>) -> Vec<Arc<Chunk>> {
    chunks.into_iter().map(Arc::new).collect()
}

/// Overlapping chunks, a cross-chunk collision, an equal-time run, a
/// static shadow, clears on one and on every entity, a second timeline.
fn nasty_chunks(other: &ComponentDescriptor) -> Vec<Arc<Chunk>> {
    arcs([
        scalar_rows_chunk(
            TEST_ENTITY,
            INDEX,
            &[(10, 1), (30, 3), (50, 5), (70, 71), (70, 72), (70, 73)],
        ),
        scalar_rows_chunk(TEST_ENTITY, INDEX, &[(20, 2), (30, 33), (40, 4)]),
        scalar_chunk_on_component("s", 0, INDEX, None, 99, other.clone()),
        scalar_rows_chunk("/other", INDEX, &[(10, 100), (40, 400)]),
        clear_chunk(TEST_ENTITY, INDEX, Some(35)),
        clear_chunk(TEST_ENTITY, INDEX, Some(55)),
        clear_chunk("/other", INDEX, Some(55)),
        clear_chunk("/two", INDEX, Some(55)),
        clear_chunk("/stale", INDEX, Some(55)),
        two_timeline_chunk(
            "/two",
            TimeType::Sequence,
            TimeType::Sequence,
            &[(15, 1500, 7), (45, 4500, 8)],
        ),
        scalar_rows_chunk("/stale", INDEX, &[(5, 1)]),
        scalar_rows_chunk("/stale", INDEX, &[(10, 2)]),
    ])
}

fn nasty_query(other: &ComponentDescriptor, not_null: bool) -> QueryExpression {
    let value = test_component();
    query(
        &[
            (TEST_ENTITY, &[value.clone(), other.clone()]),
            ("/other", std::slice::from_ref(&value)),
            ("/two", std::slice::from_ref(&value)),
            ("/stale", std::slice::from_ref(&value)),
        ],
        not_null.then_some(("/other", &value)),
    )
}

#[test]
fn gather_matches_interleave() {
    let list = |rows: &[Option<&[u8]>]| -> ArrayRef {
        Arc::new(arrow::array::ListArray::from_iter_primitive::<
            arrow::datatypes::UInt8Type,
            _,
            _,
        >(rows.iter().map(|r| {
            r.map(|bytes| bytes.iter().copied().map(Some).collect::<Vec<_>>())
        })))
    };
    let sources = [
        list(&[Some(b"ab"), Some(b"cde"), None, Some(b"f")]),
        list(&[Some(b"gh"), Some(b"")]),
        list(&[None]),
    ];
    let datatype = sources[0].data_type().clone();
    let null = new_null_array(&datatype, 1);
    let refs: Vec<&dyn Array> =
        std::iter::chain(sources.iter().map(|a| a.as_ref()), [null.as_ref()]).collect();

    let cases: &[&[Option<(usize, usize)>]] = &[
        &[],
        &[None, None],
        &[Some((0, 1)), Some((0, 2)), Some((0, 3))],
        &[
            Some((0, 0)),
            Some((0, 1)),
            None,
            None,
            Some((1, 0)),
            Some((1, 1)),
            Some((2, 0)),
            Some((2, 0)),
        ],
        &[Some((1, 1)), None, Some((0, 3)), Some((2, 0)), Some((0, 0))],
    ];
    for &indices in cases {
        let actual = gather(&datatype, &sources, indices).unwrap();
        let as_interleave: Vec<(usize, usize)> = indices
            .iter()
            .map(|i| i.unwrap_or((sources.len(), 0)))
            .collect();
        let expected = interleave(&refs, &as_interleave).unwrap();
        assert_eq!(actual.to_data(), expected.to_data(), "indices {indices:?}");
    }
}

#[test]
fn gather_of_mixed_datatypes_is_an_error() {
    let ints: ArrayRef = Arc::new(Int64Array::from(vec![1, 2]));
    let strings: ArrayRef = Arc::new(StringArray::from(vec!["a"]));
    let sources = [ints, strings];
    let cases: &[&[Option<(usize, usize)>]] = &[
        &[Some((0, 0)), Some((0, 1)), Some((1, 0))],
        &[Some((0, 1)), Some((1, 0)), Some((0, 0))],
    ];
    for &indices in cases {
        assert!(
            gather(&DataType::Int64, &sources, indices).is_err(),
            "indices {indices:?}"
        );
    }
}

#[test]
fn clear_sentinel_has_the_view_datatype() {
    let field = Arc::new(Field::new("element", DataType::Float32, false));
    let datatype = DataType::List(field);
    assert_eq!(empty_list(&datatype).data_type(), &datatype);
}

/// `QueryHandle`'s rows for `query`, in one flush.
async fn query_handle_rows(
    chunks: &[Arc<Chunk>],
    query: &QueryExpression,
    projected: &Arc<Schema>,
) -> RecordBatch {
    emit(
        RowEmitterKind::QueryHandle,
        chunks,
        query,
        projected,
        &[],
        None,
    )
    .await
    .0
}

#[tokio::test]
async fn nasty_shapes_match_query_handle_and_gc_frees_chunks() {
    let other = ComponentDescriptor::partial("other");
    let chunks = nasty_chunks(&other);
    for not_null in [false, true] {
        let query = nasty_query(&other, not_null);
        let projected = projected_schema(&chunks, &query);
        let expected = query_handle_rows(&chunks, &query, &projected).await;
        let (actual, _) = emit(
            RowEmitterKind::Columnar,
            &chunks,
            &query,
            &projected,
            &[],
            None,
        )
        .await;
        assert_same_rows(&actual, &expected);

        let (actual, removed) = emit(
            RowEmitterKind::Columnar,
            &chunks,
            &query,
            &projected,
            &[25, 25, 55],
            None,
        )
        .await;
        assert_same_rows(&actual, &expected);
        assert_eq!(
            removed[0], 1,
            "at horizon 25 the `/stale` chunk at 5 is superseded by the one at 10 and freed"
        );
        assert_eq!(
            removed[1], 0,
            "a horizon that did not advance frees nothing"
        );
    }
}

#[tokio::test]
async fn limit_rows_caps_the_output() {
    let other = ComponentDescriptor::partial("other");
    let chunks = nasty_chunks(&other);
    let query = nasty_query(&other, false);
    let projected = projected_schema(&chunks, &query);
    let expected = query_handle_rows(&chunks, &query, &projected).await;
    let (actual, _) = emit(
        RowEmitterKind::Columnar,
        &chunks,
        &query,
        &projected,
        &[25],
        Some(3),
    )
    .await;
    assert_same_rows(&actual, &expected.slice(0, 3));
}

#[test]
fn output_batches_are_cut_by_rows_or_bytes() {
    let row_bytes = [10, 10, 50, 10, 10];
    assert_eq!(next_batch_end(&row_bytes, 0, 2, 1000), 2, "row cap");
    assert_eq!(
        next_batch_end(&row_bytes, 0, 10, 60),
        3,
        "cut after the row that reaches the byte cap"
    );
    assert_eq!(
        next_batch_end(&row_bytes, 2, 10, 5),
        3,
        "a row over the byte cap still ships alone"
    );
    assert_eq!(next_batch_end(&row_bytes, 3, 10, 1000), 5, "end of grid");
}

/// A multi-row static chunk repeats only its winning row, so the winner is
/// estimated at its own size on every grid row: a big winner among small
/// rows is not averaged down, and a small winner among big rows does not
/// carry their bytes.
#[test]
fn static_cell_is_estimated_at_its_own_size() {
    let blob_len = 100_000;
    let blob_bytes = blob_len * size_of::<i64>();
    let row_bytes = |static_lens: &[usize]| {
        let other = ComponentDescriptor::partial("other");
        let mut statics = Chunk::builder(EntityPath::from(TEST_ENTITY));
        for &len in static_lens {
            let array: ArrayRef = Arc::new(Int64Array::from(vec![0; len]));
            statics = statics.with_row(RowId::new(), TimePoint::STATIC, [(other.clone(), array)]);
        }
        let chunks = arcs([
            scalar_rows_chunk(TEST_ENTITY, INDEX, &[(10, 1), (20, 2), (30, 3)]),
            statics.build().unwrap(),
        ]);
        let query = query(&[(TEST_ENTITY, &[test_component(), other])], None);
        let projected = projected_schema(&chunks, &query);

        let store = store_of(&chunks);
        let window = Window::collect(
            &store,
            &query,
            index(),
            AbsoluteTimeRange::EVERYTHING,
            &projected,
        );
        window.estimated_row_bytes(&projected)
    };

    let big_winner = row_bytes(&[1, 1, 1, blob_len]);
    assert_eq!(big_winner.len(), 3, "one estimate per grid row");
    assert!(
        big_winner.iter().all(|&bytes| bytes >= blob_bytes),
        "every row carries the {blob_bytes}-byte static winner: {big_winner:?}"
    );

    let small_winner = row_bytes(&[blob_len, blob_len, blob_len, 1]);
    assert!(
        small_winner.iter().all(|&bytes| bytes < blob_bytes / 100),
        "no row carries the {blob_bytes}-byte losers: {small_winner:?}"
    );
}

/// Only projected timelines get other-timeline values: one the query does
/// not select costs nothing per grid row.
#[test]
fn only_projected_timelines_are_built() {
    let chunks = arcs([two_timeline_chunk(
        TEST_ENTITY,
        TimeType::Sequence,
        TimeType::Sequence,
        &[(10, 100, 1), (20, 200, 2)],
    )]);
    let query = query(&[(TEST_ENTITY, &[test_component()])], None);
    let store = store_of(&chunks);
    let built = |projected: &Schema| {
        Window::collect(
            &store,
            &query,
            index(),
            AbsoluteTimeRange::EVERYTHING,
            projected,
        )
        .other_indices
        .into_iter()
        .map(|(name, _)| name)
        .collect::<Vec<_>>()
    };

    let projected = projected_schema(&chunks, &query);
    assert_eq!(built(&projected), [OTHER_INDEX]);
    let without_other = Schema::new_with_metadata(
        projected
            .fields()
            .iter()
            .filter(|f| f.name() != OTHER_INDEX)
            .cloned()
            .collect::<Vec<_>>(),
        projected.metadata().clone(),
    );
    assert!(built(&without_other).is_empty());
}

#[test]
fn queries_the_columnar_path_does_not_cover_fall_back() {
    let base = query(&[(TEST_ENTITY, &[test_component()])], None);
    assert_eq!(columnar_index(&base), Some(index()));

    let fill = QueryExpression {
        sparse_fill_strategy: SparseFillStrategy::LatestAtGlobal,
        ..base.clone()
    };
    let sampled = QueryExpression {
        using_index_values: Some(BTreeSet::from([TimeInt::new_temporal(10)])),
        ..base.clone()
    };
    let filtered = QueryExpression {
        filtered_index_values: Some(BTreeSet::from([TimeInt::new_temporal(10)])),
        ..base.clone()
    };
    let static_only = QueryExpression {
        filtered_index: None,
        ..base
    };
    for (name, query) in [
        ("fill", fill),
        ("using_index_values", sampled),
        ("filtered_index_values", filtered),
        ("static-only", static_only),
    ] {
        assert_eq!(columnar_index(&query), None, "{name}");
        let emitter = SegmentRowEmitter::new(
            re_uri::Origin::test(),
            "seg-a".into(),
            &query,
            &None,
            RowEmitterKind::Columnar,
        );
        assert_eq!(emitter.kind(), RowEmitterKind::QueryHandle, "{name}");
    }
}

/// Both paths through the real driver: two segments, several batches,
/// a window small enough to force incremental emission and GC.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn driver_output_matches_the_query_handle_path() {
    fn fixture() -> (Vec<RecordBatch>, Vec<FetchBehavior>) {
        // 8 MiB per chunk puts each chunk in its own fetch batch, so the
        // horizon advances between deliveries and both paths emit
        // incrementally.
        let chunk_infos = ["seg-a", "seg-b"]
            .iter()
            .map(|seg| {
                chunk_info_batch_sized(
                    seg,
                    Some(INDEX),
                    &[Some(10), Some(20), Some(30)],
                    &[false; 3],
                    &[Some(100); 3],
                    &[8 * 1024 * 1024; 3],
                )
            })
            .collect();
        let responses = ["seg-a", "seg-b"]
            .iter()
            .flat_map(|seg| {
                (0..3).map(move |i| {
                    let n = i64::try_from(i).unwrap();
                    FetchBehavior::Chunks(vec![(
                        scalar_chunk(seg, i, INDEX, Some((n + 1) * 10), n),
                        (*seg).to_owned(),
                    )])
                })
            })
            .collect();
        (chunk_infos, responses)
    }

    let (infos, responses) = fixture();
    let (result, store_batches, _) = run_driver_projected_with(
        RowEmitterKind::QueryHandle,
        infos,
        INDEX,
        responses,
        64 * 1024 * 1024,
        1,
        None,
        schema_with_component("value"),
    )
    .await;
    result.unwrap();

    let (infos, responses) = fixture();
    let (result, columnar_batches, _) = run_driver_projected_with(
        RowEmitterKind::Columnar,
        infos,
        INDEX,
        responses,
        64 * 1024 * 1024,
        1,
        None,
        schema_with_component("value"),
    )
    .await;
    result.unwrap();

    // Nullability and column order come from the projected schema on both
    // paths; `rerun_segment_id` there is non-nullable.
    let projected = schema_with_component("value");
    for (name, batches) in [
        ("QueryHandle", &store_batches),
        ("columnar", &columnar_batches),
    ] {
        for batch in batches {
            assert_eq!(batch.schema(), projected, "{name} batch schema");
        }
    }
    assert_eq!(
        emitted_segment_ids(&columnar_batches),
        emitted_segment_ids(&store_batches)
    );
    assert_eq!(
        emitted_component_values(&columnar_batches, "value"),
        emitted_component_values(&store_batches, "value")
    );
    assert_eq!(
        emitted_component_values(&columnar_batches, "value"),
        vec![Some(0), Some(1), Some(2), Some(0), Some(1), Some(2)]
    );
}

fn column(entity: &str, descr: &ComponentDescriptor) -> ComponentColumnSelector {
    ComponentColumnSelector::from_descriptor(EntityPath::from(entity), descr)
}

fn reference_query(
    columns: Vec<ComponentColumnSelector>,
    not_null: Option<usize>,
) -> ReferenceQuery {
    ReferenceQuery {
        index: index(),
        columns,
        range: None,
        filtered_is_not_null: not_null,
        fill_latest_at: false,
        driving: None,
    }
}

/// The `QueryExpression` that selects exactly `rq`'s columns.
fn query_expression(rq: &ReferenceQuery) -> QueryExpression {
    let mut contents: BTreeMap<EntityPath, Option<BTreeSet<_>>> = BTreeMap::new();
    for c in &rq.columns {
        contents
            .entry(c.entity_path.clone())
            .or_insert_with(|| Some(BTreeSet::new()))
            .get_or_insert_default()
            .insert(c.component_identifier().unwrap());
    }
    QueryExpression {
        view_contents: Some(ViewContentsSelector(contents)),
        filtered_index: Some(rq.index),
        filtered_is_not_null: rq.filtered_is_not_null.map(|p| rq.columns[p].clone()),
        include_tombstone_columns: true,
        ..Default::default()
    }
}

/// The emitter's rows on the columnar path, with chunks arriving as in
/// [`emit_arriving`] and emitted over `horizons`, against the reference
/// oracle: same times, and the same value or null in every cell of every
/// selected column.
async fn assert_matches_reference(
    chunks: &[Arc<Chunk>],
    arrivals: &[usize],
    rq: &ReferenceQuery,
    horizons: &[i64],
) {
    let query = query_expression(rq);
    let projected = projected_schema(chunks, &query);
    let (actual, _) = emit_arriving(
        RowEmitterKind::Columnar,
        chunks,
        arrivals,
        &query,
        &projected,
        horizons,
        None,
    )
    .await;
    let expected = reference::evaluate(chunks, rq);

    // A view without temporal data on the index has no index column.
    let Some(times) = actual.column_by_name(INDEX) else {
        assert_eq!(actual.num_rows(), 0, "rows without an index column");
        assert!(expected.is_empty(), "reference rows without temporal data");
        return;
    };
    let times = cast(times, &arrow::datatypes::DataType::Int64).unwrap();
    let times = times.as_any().downcast_ref::<Int64Array>().unwrap();
    assert_eq!(
        times.values().to_vec(),
        expected.iter().map(|r| r.time.as_i64()).collect::<Vec<_>>(),
        "grid over horizons {horizons:?}"
    );

    for (c, selector) in rq.columns.iter().enumerate() {
        let name = format!("{}:{}", selector.entity_path, selector.component);
        let list = actual
            .column_by_name(&name)
            .unwrap_or_else(|| panic!("column {name} in the output"))
            .as_any()
            .downcast_ref::<arrow::array::ListArray>()
            .unwrap();
        for (i, row) in expected.iter().enumerate() {
            let actual_cell = list.is_valid(i).then(|| list.value(i));
            let expected_cell = reference::cell_value(chunks, selector, row.cells[c]);
            assert_eq!(
                actual_cell,
                expected_cell,
                "{name} at t = {} over horizons {horizons:?}",
                row.time.as_i64()
            );
        }
    }

    let other_timelines: BTreeSet<TimelineName> = chunks
        .iter()
        .flat_map(|chunk| chunk.timelines().keys().copied())
        .filter(|timeline| *timeline != rq.index)
        .collect();
    for timeline in other_timelines {
        let expected_times: Vec<Option<i64>> = expected
            .iter()
            .map(|row| row.other_times.get(&timeline).map(|t| t.as_i64()))
            .collect();
        let Some(column) = actual.column_by_name(timeline.as_str()) else {
            assert!(
                expected_times.iter().all(Option::is_none),
                "{timeline} missing from the output"
            );
            continue;
        };
        let column = cast(column, &arrow::datatypes::DataType::Int64).unwrap();
        let column = column.as_any().downcast_ref::<Int64Array>().unwrap();
        let actual_times: Vec<Option<i64>> = (0..column.len())
            .map(|i| column.is_valid(i).then(|| column.value(i)))
            .collect();
        assert_eq!(
            actual_times, expected_times,
            "{timeline} over horizons {horizons:?}"
        );
    }
}

/// Timestamp and duration timelines, as the index or as the other
/// timeline, match `QueryHandle`, in one flush and over incremental
/// windows.
#[tokio::test]
async fn every_time_type_matches_query_handle() {
    let value = test_component();
    for (index_type, other_type) in [
        (TimeType::TimestampNs, TimeType::DurationNs),
        (TimeType::DurationNs, TimeType::TimestampNs),
    ] {
        let chunks = arcs([
            two_timeline_chunk("/a", index_type, other_type, &[(10, 100, 1), (30, 300, 3)]),
            two_timeline_chunk("/b", index_type, other_type, &[(20, 400, 2)]),
        ]);
        let query = query(
            &[
                ("/a", std::slice::from_ref(&value)),
                ("/b", std::slice::from_ref(&value)),
            ],
            None,
        );
        let projected = projected_schema(&chunks, &query);
        let expected = query_handle_rows(&chunks, &query, &projected).await;
        for horizons in [&[][..], &[15, 25][..]] {
            let (actual, _) = emit(
                RowEmitterKind::Columnar,
                &chunks,
                &query,
                &projected,
                horizons,
                None,
            )
            .await;
            assert_same_rows(&actual, &expected);
        }
    }
}

/// A `Clear`'s own column shows its flag, temporal or static, as
/// `QueryHandle` and the reference do.
#[tokio::test]
async fn clear_does_not_clear_its_own_column() {
    let flag = archetypes::Clear::descriptor_is_recursive();
    let value = test_component();
    let cases = [
        (
            arcs([
                scalar_rows_chunk("/e", INDEX, &[(10, 1), (30, 3)]),
                recursive_clear_chunk("/e", INDEX, Some(20)),
            ]),
            vec![("/e", value.clone()), ("/e", flag.clone())],
        ),
        (
            arcs([
                clear_chunk("/e", INDEX, None),
                scalar_rows_chunk("/other", INDEX, &[(10, 1), (30, 3)]),
            ]),
            vec![("/e", flag.clone()), ("/other", value.clone())],
        ),
    ];
    for (chunks, columns) in cases {
        let rq = reference_query(columns.iter().map(|(e, d)| column(e, d)).collect(), None);
        let query = query_expression(&rq);
        let projected = projected_schema(&chunks, &query);
        let expected = query_handle_rows(&chunks, &query, &projected).await;
        for horizons in [&[][..], &[15, 25][..]] {
            let (actual, _) = emit(
                RowEmitterKind::Columnar,
                &chunks,
                &query,
                &projected,
                horizons,
                None,
            )
            .await;
            assert_same_rows(&actual, &expected);
        }
        let arrivals = vec![0; chunks.len()];
        assert_matches_reference(&chunks, &arrivals, &rq, &[15, 25]).await;
    }
}

/// `SplitMix64`, so every random case replays from its seed alone.
struct SplitMix(u64);

impl SplitMix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    fn in_range(&mut self, range: std::ops::RangeInclusive<i64>) -> i64 {
        let span = usize::try_from(range.end() - range.start() + 1).unwrap();
        range.start() + i64::try_from(self.below(span)).unwrap()
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// A root, a parent/child pair and an unrelated entity, so some clears
/// land on an ancestor of a column, where they must not apply.
const RANDOM_ENTITIES: [&str; 4] = ["/", "/a", "/a/b", "/c"];

/// Few distinct times, so rows of one column collide on time often.
const RANDOM_TIMES: std::ops::RangeInclusive<i64> = 1..=6;

/// One logged row of a random segment, kept so the generator can count
/// which edge cases a case contains.
enum RandomRow {
    Data {
        entity: EntityPath,
        component: ComponentDescriptor,
        time: Option<i64>,

        /// Neither null nor an empty list.
        has_value: bool,

        /// The same row also logs a `Clear` on `entity`.
        with_clear: bool,
    },
    Clear {
        entity: EntityPath,
        time: Option<i64>,
        recursive: bool,
    },
}

struct RandomSegment {
    chunks: Vec<Arc<Chunk>>,

    /// Before which flush each chunk arrives, as in [`emit_arriving`].
    arrivals: Vec<usize>,
    rq: ReferenceQuery,
    horizons: Vec<i64>,
}

impl std::fmt::Display for RandomSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (chunk, arrival) in std::iter::zip(&self.chunks, &self.arrivals) {
            writeln!(f, "arriving before flush {arrival}:\n{chunk}")?;
        }
        let columns: Vec<String> = self.rq.columns.iter().map(column_header).collect();
        write!(
            f,
            "columns: {columns:?}\nnot_null: {:?}\nhorizons: {:?}",
            self.rq.filtered_is_not_null, self.horizons
        )
    }
}

/// 1–6 chunks over [`RANDOM_ENTITIES`], each either data on one of two
/// components (static or temporal, possibly null rows) or `Clear`s
/// (static or temporal, recursive or not), inserted in shuffled order;
/// a random subset of the logged columns, sometimes with a `not_null`; and
/// 0–3 nondecreasing horizons.
fn random_segment(rng: &mut SplitMix) -> (RandomSegment, Vec<RandomRow>) {
    let components = [test_component(), ComponentDescriptor::partial("other")];
    let clear_descr = archetypes::Clear::descriptor_is_recursive();
    let timepoint = |time: Option<i64>, other: Option<i64>| {
        let mut timepoint = TimePoint::STATIC;
        if let Some(t) = time {
            timepoint.insert(Timeline::new_sequence(INDEX), TimeInt::new_temporal(t));
        }
        if let Some(o) = other {
            timepoint.insert(
                Timeline::new_sequence(OTHER_INDEX),
                TimeInt::new_temporal(o),
            );
        }
        timepoint
    };
    let random_time = |rng: &mut SplitMix| rng.in_range(RANDOM_TIMES);
    let flag = |recursive: bool| -> ArrayRef {
        Arc::new(arrow::array::BooleanArray::from(vec![recursive]))
    };

    let mut rows = Vec::new();
    let mut chunks = Vec::new();
    let mut valued_columns_per_chunk: Vec<Vec<ComponentColumnSelector>> = Vec::new();
    let mut next_value = 0;
    let num_chunks = 1 + rng.below(6);
    for i in 0..num_chunks {
        let is_data = i == 0 || rng.chance(60);
        let entity = EntityPath::from(*rng.pick(&RANDOM_ENTITIES));
        let is_static = rng.chance(25);
        let has_other = !is_static && rng.chance(40);
        let num_rows = 1 + rng.below(3);
        let mut builder = Chunk::builder(entity.clone());
        let mut valued_columns = Vec::new();
        if is_data {
            let component = rng.pick(&components).clone();
            for row in 0..num_rows {
                let time = (!is_static).then(|| random_time(rng));
                let other = has_other.then(|| rng.in_range(1..=20));
                let is_null = row > 0 && rng.chance(20);
                next_value += 1;
                // The first row always holds a value, so some column can be selected.
                let shape = if i == 0 && row == 0 { 9 } else { rng.below(10) };
                let values = match shape {
                    0 => vec![],
                    1 => vec![next_value, -next_value],
                    _ => vec![next_value],
                };
                let with_clear = rng.chance(10);
                let recursive = rng.chance(50);
                let has_value = !is_null && !values.is_empty();
                if has_value {
                    valued_columns.push(column(&entity.to_string(), &component));
                }
                let array: Option<ArrayRef> =
                    (!is_null).then(|| Arc::new(Int64Array::from(values)) as _);
                builder = builder.with_sparse_row(
                    RowId::new(),
                    timepoint(time, other),
                    [
                        (component.clone(), array),
                        (clear_descr.clone(), with_clear.then(|| flag(recursive))),
                    ],
                );
                rows.push(RandomRow::Data {
                    entity: entity.clone(),
                    component: component.clone(),
                    time,
                    has_value,
                    with_clear,
                });
                if with_clear {
                    rows.push(RandomRow::Clear {
                        entity: entity.clone(),
                        time,
                        recursive,
                    });
                }
            }
        } else {
            for _ in 0..num_rows {
                let time = (!is_static).then(|| random_time(rng));
                let other = has_other.then(|| rng.in_range(1..=20));
                let recursive = rng.chance(50);
                builder = builder.with_row(
                    RowId::new(),
                    timepoint(time, other),
                    [(clear_descr.clone(), flag(recursive))],
                );
                rows.push(RandomRow::Clear {
                    entity: entity.clone(),
                    time,
                    recursive,
                });
            }
        }
        chunks.push(Arc::new(builder.build().unwrap()));
        valued_columns_per_chunk.push(valued_columns);
    }

    // `RowId`s follow creation order; insertion order must not matter.
    let mut order: Vec<usize> = (0..chunks.len()).collect();
    for i in (1..order.len()).rev() {
        order.swap(i, rng.below(i + 1));
    }
    let chunks: Vec<Arc<Chunk>> = order.iter().map(|&i| Arc::clone(&chunks[i])).collect();

    // The store leaves a component whose values are all null or empty out
    // of the view, which the reference does not model.
    let mut logged: Vec<ComponentColumnSelector> = rows
        .iter()
        .filter_map(|row| match row {
            RandomRow::Data {
                entity,
                component,
                has_value: true,
                ..
            } => Some(column(&entity.to_string(), component)),
            RandomRow::Data { .. } | RandomRow::Clear { .. } => None,
        })
        .collect();
    logged.sort_by_key(|c| (c.entity_path.clone(), c.component.clone()));
    logged.dedup();
    let mut columns: Vec<_> = logged.iter().filter(|_| rng.chance(75)).cloned().collect();
    if columns.is_empty() {
        columns.push(rng.pick(&logged).clone());
    }
    let not_null = rng.chance(30).then(|| rng.below(columns.len()));

    let mut horizons: Vec<i64> = (0..rng.below(4))
        .map(|_| rng.in_range(RANDOM_TIMES.start() - 1..=RANDOM_TIMES.end() + 1))
        .collect();
    horizons.sort_unstable();

    // A chunk must arrive before the first flush whose horizon reaches its
    // earliest time; static chunks arrive first.
    let mut arrivals: Vec<usize> = chunks
        .iter()
        .map(|chunk| {
            let deadline = match chunk.timelines().get(&index()) {
                Some(times) => {
                    let earliest = times.times_raw().iter().min().copied().unwrap_or(i64::MAX);
                    horizons
                        .iter()
                        .position(|&h| earliest <= h)
                        .unwrap_or(horizons.len())
                }
                None => 0,
            };
            rng.below(deadline + 1)
        })
        .collect();
    // TODO(RR-6157): an emitter builds its view from the chunks that have
    // arrived, so a `Clear` emitted before its column's first chunk is
    // lost; until the view comes from the plan, every selected column is
    // in the view from the first flush.
    for selected in &columns {
        if let Some(pos) = order
            .iter()
            .position(|&gen_idx| valued_columns_per_chunk[gen_idx].contains(selected))
        {
            arrivals[pos] = 0;
        }
    }

    let segment = RandomSegment {
        chunks,
        arrivals,
        rq: reference_query(columns, not_null),
        horizons,
    };
    (segment, rows)
}

/// Every edge case the random segments must keep producing, so a generator
/// change that stops producing one fails the test instead of silently
/// weakening it.
const EDGE_CASES: [&str; 11] = [
    "same time in one column",
    "temporal clear on a column",
    "recursive clear on an ancestor",
    "static clear on a column",
    "static clear shadowing temporal clears",
    "static value shadowing temporal",
    "value and clear in one row",
    "not_null",
    "incremental",
    "late arrival",
    "other timeline",
];

/// The [`EDGE_CASES`] `segment` contains.
fn edge_cases(segment: &RandomSegment, rows: &[RandomRow]) -> Vec<&'static str> {
    let selected: Vec<(&EntityPath, &str)> = segment
        .rq
        .columns
        .iter()
        .map(|c| (&c.entity_path, c.component.as_str()))
        .collect();
    let mut times_per_column: BTreeMap<(&EntityPath, &str), Vec<i64>> = BTreeMap::new();
    let mut has_static = BTreeSet::new();
    for row in rows {
        if let RandomRow::Data {
            entity,
            component,
            time,
            ..
        } = row
        {
            let key = (entity, component.component.as_str());
            match time {
                Some(t) => times_per_column.entry(key).or_default().push(*t),
                None => {
                    has_static.insert(key);
                }
            }
        }
    }
    let has_clear = |entity: &EntityPath, want_static: bool| {
        rows.iter().any(|row| {
            matches!(row, RandomRow::Clear { entity: e, time, .. }
                if e == entity && time.is_none() == want_static)
        })
    };
    let on_a_column = |want_static: bool| {
        selected
            .iter()
            .any(|(column, _)| has_clear(column, want_static))
    };

    let hits = [
        selected.iter().any(|key| {
            times_per_column
                .get(key)
                .is_some_and(|times| times.iter().collect::<BTreeSet<_>>().len() < times.len())
        }),
        on_a_column(false),
        rows.iter().any(|row| {
            matches!(row, RandomRow::Clear { entity, recursive: true, .. }
                if selected.iter().any(|(column, _)| column.is_descendant_of(entity)))
        }),
        on_a_column(true),
        selected
            .iter()
            .any(|(column, _)| has_clear(column, true) && has_clear(column, false)),
        selected
            .iter()
            .any(|key| has_static.contains(key) && times_per_column.contains_key(key)),
        rows.iter().any(|row| {
            matches!(row, RandomRow::Data { entity, component, with_clear: true, .. }
                if selected.contains(&(entity, component.component.as_str())))
        }),
        segment.rq.filtered_is_not_null.is_some(),
        !segment.horizons.is_empty(),
        segment.arrivals.iter().any(|&a| a > 0),
        segment.chunks.iter().any(|chunk| {
            chunk
                .timelines()
                .contains_key(&TimelineName::try_new(OTHER_INDEX).unwrap())
        }),
    ];
    std::iter::zip(EDGE_CASES, hits)
        .filter(|(_, hit)| *hit)
        .map(|(name, _)| name)
        .collect()
}

/// Prints the whole case when one panics, since the assertion message only
/// names the failing cell.
struct ReportOnPanic<'a>(&'a str);

impl Drop for ReportOnPanic<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("failing random case:\n{}", self.0);
        }
    }
}

/// Random segments, emitted in one shot and across random horizons, match
/// the reference oracle cell by cell.
#[tokio::test]
async fn random_segments_match_the_reference() {
    const CASES: u64 = 5000;
    const SEED: u64 = 0x5EED;

    let mut counts: BTreeMap<&str, usize> = EDGE_CASES.iter().map(|&name| (name, 0)).collect();
    for case in 0..CASES {
        let mut rng = SplitMix(SEED.wrapping_add(case));
        let (segment, rows) = random_segment(&mut rng);
        for name in edge_cases(&segment, &rows) {
            *counts.get_mut(name).unwrap() += 1;
        }

        let description = format!("seed {:#x}\n{segment}", SEED.wrapping_add(case));
        let _report = ReportOnPanic(&description);
        assert_matches_reference(
            &segment.chunks,
            &segment.arrivals,
            &segment.rq,
            &segment.horizons,
        )
        .await;
    }

    for (name, count) in counts {
        assert!(count >= 10, "only {count} of {CASES} cases have {name}");
    }
}
