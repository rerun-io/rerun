use crate::RecordBatchTestExt as _;
use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, concat_record_batches,
};
use crate::tests::find_chunks::{find_chunks_ids, range_request};
use crate::utils::client::{RecordedRequest, TestClient, create_test_client};
use crate::utils::rerun::{TWO_TIMELINE_ENTITY_PATH, next_chunk_id_generator};
use arrow::array::RecordBatch;
use datafusion::datasource::{MemTable, TableProvider as _};
use datafusion::execution::SessionState;
use datafusion::physical_plan::ExecutionPlanProperties as _;
use datafusion::prelude::{Expr, SessionConfig, SessionContext, col, lit};
use datafusion::scalar::ScalarValue;
use futures::{StreamExt as _, TryStreamExt as _};
use itertools::Itertools as _;
use re_datafusion::{
    DataframeClientAPI, DataframeQueryTableProvider, NoOpObjectStoreAuthenticator,
};
use re_log_types::example_components::MyPoints;
use re_log_types::{EntityPath, EntryId};
use re_protos::cloud::v1alpha1::ext::{
    ChunkMode, ComponentSelection, ContentSelection, EntitySelection, FillMode, SegmentSelection,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_types_core::ChunkId;
use std::collections::BTreeSet;
use std::sync::Arc;

pub async fn query_dataset_simple_filter(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::multi_chunked_entities(
                "my_segment_id1",
                &["my/entity", "my/other/entity"],
            ),
            LayerDefinition::multi_chunked_entities("my_segment_id2", &["my/entity"]),
            LayerDefinition::multi_chunked_entities(
                "my_segment_id3",
                &["my/entity", "another/one", "yet/another/one"],
            ),
        ],
    );

    let dataset_name = "dataset";
    let dataset_entry = service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    let client = create_test_client(service).await;

    let ctx = SessionContext::default();
    let state = ctx.state();

    let tests = vec![
        (lit(true), "default"),
        (
            col("rerun_segment_id").eq(lit("my_segment_id2")),
            "seg_id_eq",
        ),
        (col("frame_nr").eq(lit(50)), "frame_nr_eq"),
        // 2-sided range — the path that `replace_time_in_query` +
        // `merge_queries_and` pair with a latest_at, and which the
        // post-pushdown scrub strips when `sparse_fill_strategy=None`. The
        // snapshot exercises end-to-end row-correctness for the patched path.
        (
            col("frame_nr")
                .gt_eq(lit(40))
                .and(col("frame_nr").lt_eq(lit(60))),
            "frame_nr_range",
        ),
    ];

    // Run every filter case under both `SparseFillStrategy` variants so any
    // future operator added to `tests` automatically picks up fill-mode
    // coverage. Per-operator regressions under fill (e.g. a refactor of
    // `replace_time_in_query`'s `synthesize_latest_at` gating that drops
    // `latest_at` for the wrong arm) surface here without needing a parallel
    // test harness.
    //
    // Snapshot naming: the `None` variant keeps the historical
    // `simple_dataset_{name}` prefix so existing snapshots are unaffected;
    // the `LatestAtGlobal` variant appends `_fill_latest_at` for the new
    // snapshots.
    for (sparse_fill_strategy, suffix) in [
        (re_chunk_store::SparseFillStrategy::None, ""),
        (
            re_chunk_store::SparseFillStrategy::LatestAtGlobal,
            "_fill_latest_at",
        ),
    ] {
        let query = re_chunk_store::QueryExpression {
            view_contents: Some(std::iter::once((EntityPath::from("my/entity"), None)).collect()),
            filtered_index: Some("frame_nr".into()),
            sparse_fill_strategy,
            ..Default::default()
        };

        let table_provider = DataframeQueryTableProvider::new_from_client(
            client.clone(),
            dataset_entry.details.id,
            &query,
            &[] as &[&str],
            None,
            None,       // arrow_schema — let the provider fetch it
            None,       // trace_headers
            Vec::new(), // metrics_collectors
            Arc::new(NoOpObjectStoreAuthenticator::default()),
        )
        .await
        .unwrap();

        for (filter, snapshot_name) in &tests {
            query_dataset_snapshot(
                &table_provider,
                &ctx,
                &state,
                filter.clone(),
                &format!("simple_dataset_{snapshot_name}{suffix}"),
            )
            .await;
        }
    }
}

/// Regression guard for the post-pushdown `latest_at` scrub in
/// [`DataframeQueryTableProvider::scan`].
///
/// Drives an identical 2-sided range filter (`frame_nr ∈ [40, 60]`) through
/// the table provider twice — once with `SparseFillStrategy::None` and once
/// with `SparseFillStrategy::LatestAtGlobal` — and snapshots both row sets.
///
/// What this protects against:
///
/// - **Correctness for the patched, non-fill path**: the
///   `frame_nr_range_no_fill` snapshot pins the exact rows returned after
///   the scrub. If the scrub ever drops `range` (or otherwise corrupts the
///   request), this snapshot will diff.
/// - **Behavioral fidelity of the fill path**: the
///   `frame_nr_range_fill_latest_at` snapshot pins the rows returned when
///   `LatestAtGlobal` is requested. The scrub MUST NOT fire here, so the
///   request retains `latest_at` and the server still emits the
///   latest-at-fill rows the user asked for. If the scrub ever runs
///   unconditionally, fill-mode rows disappear and this snapshot diffs.
/// - **The two snapshots must remain non-identical** (fill mode produces
///   strictly more rows than the pure range scan). An explicit assertion
///   enforces this.
pub async fn query_dataset_range_filter_with_and_without_latest_at_fill(
    service: impl RerunCloudService,
) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::multi_chunked_entities(
                "my_segment_id1",
                &["my/entity", "my/other/entity"],
            ),
            LayerDefinition::multi_chunked_entities("my_segment_id2", &["my/entity"]),
            LayerDefinition::multi_chunked_entities(
                "my_segment_id3",
                &["my/entity", "another/one", "yet/another/one"],
            ),
        ],
    );

    let dataset_name = "dataset";
    let dataset_entry = service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    let client = create_test_client(service).await;

    // The same 2-sided range that exercises `merge_queries_and`, used in
    // both the no-fill and the fill-latest-at variants below.
    let range_filter = col("frame_nr")
        .gt_eq(lit(40))
        .and(col("frame_nr").lt_eq(lit(60)));

    // Variant 1: sparse_fill_strategy=None (default). The post-pushdown
    // scrub fires; the server sees a pure range request.
    let no_fill_query = re_chunk_store::QueryExpression {
        view_contents: Some(std::iter::once((EntityPath::from("my/entity"), None)).collect()),
        filtered_index: Some("frame_nr".into()),
        sparse_fill_strategy: re_chunk_store::SparseFillStrategy::None,
        ..Default::default()
    };
    let no_fill_provider = DataframeQueryTableProvider::new_from_client(
        client.clone(),
        dataset_entry.details.id,
        &no_fill_query,
        &[] as &[&str],
        None,
        None,
        None,
        Vec::new(),
        Arc::new(NoOpObjectStoreAuthenticator::default()),
    )
    .await
    .unwrap();
    query_dataset_count_and_snapshot(
        &no_fill_provider,
        range_filter.clone(),
        "frame_nr_range_no_fill",
    )
    .await;

    // Variant 2: sparse_fill_strategy=LatestAtGlobal. The scrub MUST be
    // gated off here; the server needs `latest_at` to drive fill.
    let fill_query = re_chunk_store::QueryExpression {
        view_contents: Some(std::iter::once((EntityPath::from("my/entity"), None)).collect()),
        filtered_index: Some("frame_nr".into()),
        sparse_fill_strategy: re_chunk_store::SparseFillStrategy::LatestAtGlobal,
        ..Default::default()
    };
    let fill_provider = DataframeQueryTableProvider::new_from_client(
        client,
        dataset_entry.details.id,
        &fill_query,
        &[] as &[&str],
        None,
        None,
        None,
        Vec::new(),
        Arc::new(NoOpObjectStoreAuthenticator::default()),
    )
    .await
    .unwrap();
    query_dataset_count_and_snapshot(
        &fill_provider,
        range_filter,
        "frame_nr_range_fill_latest_at",
    )
    .await;
}

async fn query_dataset_count_and_snapshot<T: DataframeClientAPI>(
    table_provider: &DataframeQueryTableProvider<T>,
    filter: Expr,
    snapshot_name: &str,
) {
    let ctx = SessionContext::default();
    let state = ctx.state();
    let plan = table_provider
        .scan(&state, None, &[filter], None)
        .await
        .unwrap();

    let num_partitions = plan.output_partitioning().partition_count();
    let results = (0..num_partitions)
        .map(|partition| plan.execute(partition, ctx.task_ctx()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    let stream = futures::stream::iter(results);
    let results: Vec<RecordBatch> = stream
        .flat_map(|stream| stream)
        .try_collect()
        .await
        .unwrap();
    let results = concat_record_batches(&results);

    insta::assert_snapshot!(
        format!("{snapshot_name}_schema"),
        results.format_schema_snapshot()
    );

    let filtered_results = results.horizontally_sorted().auto_sort_rows().unwrap();
    insta::assert_snapshot!(
        format!("{snapshot_name}_data"),
        filtered_results.format_snapshot(false)
    );
}

pub async fn query_dataset_with_limit(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::multi_chunked_entities(
                "my_segment_id1",
                &["my/entity", "my/other/entity"],
            ),
            LayerDefinition::multi_chunked_entities("my_segment_id2", &["my/entity"]),
            LayerDefinition::multi_chunked_entities(
                "my_segment_id3",
                &["my/entity", "another/one", "yet/another/one"],
            ),
        ],
    );

    let dataset_name = "dataset";
    let dataset_entry = service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;

    let client = create_test_client(service).await;
    let query = re_chunk_store::QueryExpression {
        view_contents: Some(std::iter::once((EntityPath::from("my/entity"), None)).collect()),
        filtered_index: Some("frame_nr".into()),
        ..Default::default()
    };

    let table_provider = DataframeQueryTableProvider::new_from_client(
        client,
        dataset_entry.details.id,
        &query,
        &[] as &[&str],
        None,
        None,       // arrow_schema — let the provider fetch it
        None,       // trace_headers
        Vec::new(), // metrics_collectors
        Arc::new(NoOpObjectStoreAuthenticator::default()),
    )
    .await
    .unwrap();

    // We need to set 1 target partition, otherwise we will not get exactly the limit we expect
    let config = SessionConfig::default().with_target_partitions(1);
    let ctx = SessionContext::new_with_config(config);
    let state = ctx.state();

    // First, get the total row count without any limit
    let total_rows = execute_with_limit(&table_provider, &ctx, &state, None).await;
    assert!(
        total_rows > 1,
        "Test requires more than 1 row to be meaningful, got {total_rows}"
    );

    // Limit of 1 should return exactly 1 row
    let rows = execute_with_limit(&table_provider, &ctx, &state, Some(1)).await;
    assert_eq!(rows, 1, "limit=1 should return exactly 1 row");

    // Limit smaller than total should return exactly that many rows
    let small_limit = total_rows / 2;
    let rows = execute_with_limit(&table_provider, &ctx, &state, Some(small_limit)).await;
    assert_eq!(
        rows, small_limit,
        "limit={small_limit} should return exactly {small_limit} rows"
    );

    // Limit equal to total should return all rows
    let rows = execute_with_limit(&table_provider, &ctx, &state, Some(total_rows)).await;
    assert_eq!(
        rows, total_rows,
        "limit={total_rows} should return all {total_rows} rows"
    );

    // Limit larger than total should return all rows
    let rows = execute_with_limit(&table_provider, &ctx, &state, Some(total_rows + 100)).await;
    assert_eq!(
        rows, total_rows,
        "limit larger than total should return all {total_rows} rows"
    );
}

async fn execute_with_limit<T: DataframeClientAPI>(
    table_provider: &DataframeQueryTableProvider<T>,
    ctx: &SessionContext,
    state: &SessionState,
    limit: Option<usize>,
) -> usize {
    let plan = table_provider
        .scan(state, None, &[lit(true)], limit)
        .await
        .unwrap();

    let num_partitions = plan.output_partitioning().partition_count();
    let results: Vec<_> = (0..num_partitions)
        .map(|partition| plan.execute(partition, ctx.task_ctx()))
        .try_collect()
        .unwrap();

    let stream = futures::stream::iter(results);

    let results: Vec<RecordBatch> = stream
        .flat_map(|stream| stream)
        .try_collect()
        .await
        .unwrap();

    results.iter().map(|batch| batch.num_rows()).sum()
}

/// A single scan can expand into one `QueryDatasetRequest` per branch of an
/// `OR` of index ranges (`apply_filter_expr_to_queries`), and the scan issues
/// those requests concurrently. This pins the three properties that fan-out
/// must preserve:
///
/// 1. **Completeness** — every branch is issued as its own `query_dataset`
///    request; the fan-out drops none of them.
/// 2. **Determinism** — responses are collected out of order and deduplicated
///    by chunk id, so the identical query run twice must return identical rows.
/// 3. **Deduplication** — a redundant range that selects only chunks another
///    branch already covers must not add or duplicate rows.
///
/// Frames live at `{10, 20, 30, 40, 50, 60, 70, 80, 90}` in three chunks per
/// entity (one per `base_time ∈ {0, 30, 60}`; see `multi_chunked_entities`).
/// The ranges are chosen against those chunk boundaries:
///
/// - `A = [10, 30]` selects the `{10, 20, 30}` chunk.
/// - `B = [50, 70]` selects the `{40, 50, 60}` and `{70, 80, 90}` chunks.
/// - `C = [55, 75]` selects the same two chunks as `B` — redundant, so it
///   exercises dedup without widening the selected chunk set.
pub async fn query_dataset_or_of_ranges_fans_out(service: impl RerunCloudService) {
    query_dataset_or_of_ranges_fans_out_impl(service, true).await;
}

/// [`query_dataset_or_of_ranges_fans_out`] on the SDK's `query_dataset` path: the same fixture,
/// fan-out, determinism and dedup assertions, forced onto the `query_dataset` RPC via
/// `DataframeQueryTableProvider::with_find_chunks_support(false)`. Both servers still implement
/// the old RPC through their adapters, so the same fixture serves; the snapshot name is reused
/// verbatim.
///
/// TODO(RR-5817): remove with the SDK's `query_dataset` path.
pub async fn query_dataset_or_of_ranges_fans_out_query_dataset_path(
    service: impl RerunCloudService,
) {
    query_dataset_or_of_ranges_fans_out_impl(service, false).await;
}

async fn query_dataset_or_of_ranges_fans_out_impl(
    service: impl RerunCloudService,
    supports_find_chunks: bool,
) {
    let service = Arc::new(service);

    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [
            LayerDefinition::multi_chunked_entities("my_segment_id1", &["my/entity"]),
            LayerDefinition::multi_chunked_entities("my_segment_id2", &["my/entity"]),
            LayerDefinition::multi_chunked_entities("my_segment_id3", &["my/entity"]),
        ],
    );

    let dataset_name = "dataset";
    let dataset_entry = service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;
    // Keep the temp RRD files alive: some backends re-read them during `query_dataset`.
    let _data_sources = data_sources_def;

    let query = re_chunk_store::QueryExpression {
        view_contents: Some(std::iter::once((EntityPath::from("my/entity"), None)).collect()),
        filtered_index: Some("frame_nr".into()),
        ..Default::default()
    };

    let range = |lo: i64, hi: i64| {
        col("frame_nr")
            .gt_eq(lit(lo))
            .and(col("frame_nr").lt_eq(lit(hi)))
    };
    let a = range(10, 30);
    let b = range(50, 70);
    let c = range(55, 75);

    let filter_three = a.clone().or(b.clone()).or(c);
    let filter_two = a.or(b);

    // 1. Completeness: three OR branches fan out to exactly three requests.
    let (rows_three, requests_three) = scan_collect_rows(
        &service,
        dataset_entry.details.id,
        &query,
        filter_three.clone(),
        supports_find_chunks,
    )
    .await;
    assert_eq!(
        requests_three, 3,
        "an OR of three ranges must fan out to exactly three query_dataset requests"
    );
    assert!(
        !rows_three.is_empty(),
        "the ranges should have selected some rows"
    );

    // 2. Determinism: out-of-order collection + dedup must be stable across runs.
    let (rows_three_again, _) = scan_collect_rows(
        &service,
        dataset_entry.details.id,
        &query,
        filter_three,
        supports_find_chunks,
    )
    .await;
    assert_eq!(
        rows_three, rows_three_again,
        "concurrent fan-out must produce identical rows across runs"
    );

    // 3. Dedup: dropping the redundant overlapping branch must not change rows.
    //    If dedup regressed, `C`'s chunks would be fetched twice and their rows
    //    would be duplicated, making `rows_three` strictly larger than `rows_two`.
    let (rows_two, requests_two) = scan_collect_rows(
        &service,
        dataset_entry.details.id,
        &query,
        filter_two,
        supports_find_chunks,
    )
    .await;
    assert_eq!(
        requests_two, 2,
        "an OR of two ranges must fan out to exactly two range requests"
    );
    assert_eq!(
        rows_three, rows_two,
        "a redundant overlapping range must not add or duplicate rows"
    );

    insta::assert_snapshot!("or_of_ranges_fanout_data", rows_three);
}

/// Run [`DataframeQueryTableProvider::scan`] for `filter` on a fresh
/// [`TestClient`], returning the canonically-sorted row snapshot and the number
/// of range requests the scan emitted (a `FindChunks` `Range` request on the new path, or any
/// `Legacy` request on the legacy path, since the legacy protocol has no separate mode tag).
///
/// A fresh client per call keeps the recorded request count scoped to this one
/// scan.
async fn scan_collect_rows<T: RerunCloudService>(
    service: &Arc<T>,
    dataset_id: EntryId,
    query: &re_chunk_store::QueryExpression,
    filter: Expr,
    supports_find_chunks: bool,
) -> (String, usize) {
    let client = TestClient::new(Arc::clone(service));
    let provider = DataframeQueryTableProvider::new_from_client(
        client.clone(),
        dataset_id,
        query,
        &[] as &[&str],
        None,
        None,
        None,
        Vec::new(),
        Arc::new(NoOpObjectStoreAuthenticator::default()),
    )
    .await
    .expect("build the table provider")
    .with_find_chunks_support(supports_find_chunks);

    let ctx = SessionContext::default();
    let plan = provider
        .scan(&ctx.state(), None, &[filter], None)
        .await
        .unwrap();

    let num_partitions = plan.output_partitioning().partition_count();
    let results: Vec<_> = (0..num_partitions)
        .map(|partition| plan.execute(partition, ctx.task_ctx()))
        .try_collect()
        .unwrap();
    let results: Vec<RecordBatch> = futures::stream::iter(results)
        .flat_map(|stream| stream)
        .try_collect()
        .await
        .unwrap();
    let results = concat_record_batches(&results);
    let sorted = results.horizontally_sorted().auto_sort_rows().unwrap();

    let requests = client
        .recorded_requests
        .lock()
        .iter()
        .filter(|request| match request {
            RecordedRequest::FindChunks(request) => matches!(request.mode, ChunkMode::Range { .. }),
            RecordedRequest::QueryDataset(_) => !supports_find_chunks,
        })
        .count();
    (sorted.format_snapshot(false), requests)
}

/// An `OR` of index ranges where one branch selects *nothing* still fans out
/// into one `query_dataset` request per branch — and the empty branch has to
/// come back with a schema the populated branch can be concatenated with
/// (`compute_unique_chunk_info_ids` concats all fan-out responses positionally).
///
/// A server that answers an empty result with a fixed fallback batch, while
/// answering a populated one with a projection-dependent schema, breaks that
/// concatenation with e.g. `It is not possible to concatenate arrays of
/// different data types (Utf8, Boolean)`.
///
/// The projection is part of the repro: projecting a single component column
/// narrows `fuzzy_descriptors`, which is what makes the populated response's
/// column set differ from the fallback's.
pub async fn query_dataset_or_with_empty_branch_and_projection(service: impl RerunCloudService) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::multi_chunked_entities(
            "my_segment_id1",
            &["my/entity"],
        )],
    );

    let dataset_name = "dataset";
    let dataset_entry = service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;
    let _data_sources = data_sources_def;

    let client = create_test_client(service).await;

    let query = re_chunk_store::QueryExpression {
        view_contents: Some(std::iter::once((EntityPath::from("my/entity"), None)).collect()),
        filtered_index: Some("frame_nr".into()),
        ..Default::default()
    };

    let table_provider = DataframeQueryTableProvider::new_from_client(
        client.clone(),
        dataset_entry.details.id,
        &query,
        &[] as &[&str],
        None,
        None,       // arrow_schema — let the provider fetch it
        None,       // trace_headers
        Vec::new(), // metrics_collectors
        Arc::new(NoOpObjectStoreAuthenticator::default()),
    )
    .await
    .unwrap();

    // Project the index plus exactly one component column, mirroring
    // `select(col(index), col(blob))` from the Python API.
    let schema = table_provider.schema();
    let index_idx = schema.index_of("frame_nr").unwrap();
    // By name, not by position: the dataset schema's field order is not stable across runs.
    let component_idx = schema
        .index_of("/my/entity:example.MyPoints:colors")
        .unwrap();
    let projection = vec![index_idx, component_idx];

    let range = |lo: i64, hi: i64| {
        col("frame_nr")
            .gt_eq(lit(lo))
            .and(col("frame_nr").lt_eq(lit(hi)))
    };

    // Frames live at `{10, …, 90}`, so this branch matches nothing.
    let populated = range(10, 30);
    let empty = range(-1000, -900);

    let count = async |filter: Expr| -> usize {
        let ctx = SessionContext::default();
        let state = ctx.state();
        let plan = table_provider
            .scan(&state, Some(&projection), &[filter], None)
            .await
            .unwrap();

        let num_partitions = plan.output_partitioning().partition_count();
        let results: Vec<_> = (0..num_partitions)
            .map(|partition| plan.execute(partition, ctx.task_ctx()))
            .try_collect()
            .unwrap();

        let batches: Vec<RecordBatch> = futures::stream::iter(results)
            .flat_map(|stream| stream)
            .try_collect()
            .await
            .unwrap();

        batches.iter().map(|batch| batch.num_rows()).sum()
    };

    let rows_populated = count(populated.clone()).await;
    assert!(
        rows_populated > 0,
        "the populated branch should have selected some rows"
    );
    assert_eq!(count(empty.clone()).await, 0);

    // The empty branch adds nothing, but it must not break the fan-out either.
    assert_eq!(count(populated.or(empty)).await, rows_populated);
}

/// A projected read with no `view_contents` selects only the projected entities and
/// components, so the server can skip the chunks of every other component.
pub async fn query_dataset_projection_narrows_components_without_view_contents(
    service: impl RerunCloudService,
) {
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        1,
        [LayerDefinition::multi_chunked_entities(
            "my_segment_id1",
            &["my/entity", "my/other/entity"],
        )],
    );

    let dataset_name = "dataset";
    let dataset_entry = service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;
    let _data_sources = data_sources_def;

    let client = create_test_client(service).await;
    let query = re_chunk_store::QueryExpression {
        filtered_index: Some("frame_nr".into()),
        ..Default::default()
    };
    let table_provider = DataframeQueryTableProvider::new_from_client(
        client.clone(),
        dataset_entry.details.id,
        &query,
        &[] as &[&str],
        None,
        None,
        None,
        Vec::new(),
        Arc::new(NoOpObjectStoreAuthenticator::default()),
    )
    .await
    .expect("build the table provider");

    let schema = table_provider.schema();
    let projection = vec![
        schema.index_of("frame_nr").expect("index column"),
        schema
            .index_of("/my/entity:example.MyPoints:colors")
            .expect("component column"),
    ];
    let ctx = SessionContext::default();
    table_provider
        .scan(&ctx.state(), Some(&projection), &[], None)
        .await
        .expect("plan the scan");

    let expected = ContentSelection::Explicit(vec![EntitySelection {
        entity_path: EntityPath::from("my/entity"),
        components: ComponentSelection::Explicit(
            std::iter::once(MyPoints::descriptor_colors().component.to_string()).collect(),
        ),
    }]);
    let requests = client.recorded_requests.lock().clone();
    assert!(!requests.is_empty(), "the scan must send a request");
    for request in requests {
        let RecordedRequest::FindChunks(request) = request else {
            panic!("the provider must send find_chunks: {request:?}");
        };
        assert_eq!(
            request.contents, expected,
            "the request must select only the projected entity and component"
        );
    }
}

// ---

async fn query_dataset_snapshot<T: DataframeClientAPI>(
    table_provider: &DataframeQueryTableProvider<T>,
    ctx: &SessionContext,
    state: &SessionState,
    filter: Expr,
    snapshot_name: &str,
) {
    let plan = table_provider
        .scan(state, None, &[filter], None)
        .await
        .unwrap();

    let num_partitions = plan.output_partitioning().partition_count();
    let results: Vec<_> = (0..num_partitions)
        .map(|partition| plan.execute(partition, ctx.task_ctx()))
        .try_collect()
        .unwrap();

    let stream = futures::stream::iter(results);

    let results: Vec<RecordBatch> = stream
        .flat_map(|stream| stream)
        .try_collect()
        .await
        .unwrap();

    let results = concat_record_batches(&results);

    insta::assert_snapshot!(
        format!("{snapshot_name}_schema"),
        results.format_schema_snapshot()
    );

    // these columns are not stable, so we cannot snapshot them
    let filtered_results = results.horizontally_sorted().auto_sort_rows().unwrap();

    insta::assert_snapshot!(
        format!("{snapshot_name}_data"),
        filtered_results.format_snapshot(false)
    );
}

// ---

/// The reader index `T` of the two-timeline fixture.
const TWO_TIMELINE_READER_INDEX: &str = "frame_nr";

/// The foreign timeline `U` of the two-timeline fixture: the one a filter can select on while
/// the reader still walks `T`.
const TWO_TIMELINE_FOREIGN_INDEX: &str = "timestamp";

/// The `tuid_prefix` of the first two-timeline segment; each further segment adds one.
const TWO_TIMELINE_TUID_PREFIX: u64 = 40;

/// The chunk ids `create_two_timeline_recording` assigns for `tuid_prefix`: three temporal
/// chunks, then one static chunk.
fn two_timeline_chunk_ids(tuid_prefix: u64) -> BTreeSet<ChunkId> {
    let mut next_chunk_id = next_chunk_id_generator(tuid_prefix);
    (0..4).map(|_| next_chunk_id()).collect()
}

/// A `timestamp` literal, written in milliseconds to match the fixture's row table.
fn timestamp_ms(milliseconds: i64) -> Expr {
    lit(ScalarValue::TimestampNanosecond(
        Some(milliseconds * 1_000_000),
        None,
    ))
}

/// Checks that chunk selection preserves the result of filtering complete dataframe rows for
/// a fixture whose temporal data has both `frame_nr` and `timestamp`.
///
/// The reader builds rows on `frame_nr`. When chunk selection uses `timestamp`, its bounds must
/// retain the chunks needed for those rows. Chunk starts on `timestamp` must not determine when
/// the reader can emit rows on `frame_nr`.
///
/// The oracle is a DataFusion [`MemTable`] over the *unfiltered* rows, filtered again in
/// memory. It has to re-derive the answer from unfiltered data: both servers answer
/// `query_dataset` through `adapt_legacy` and the same chunk selection `find_chunks` uses, so
/// using one protocol path's output as the other's expected value would compare a thing to
/// itself and pass however wrong both are.
///
/// The cases, all with reader index `T` = `frame_nr` and foreign timeline `U` = `timestamp`:
///
/// - a `U`-only predicate without fill, where `U` becomes the selection timeline and the server
///   prunes on it;
/// - the same predicate with `SparseFillStrategy::LatestAtGlobal`, where `T` must stay the
///   selection timeline (fill is the only case where the pushdown differs). Under fill a row's
///   `timestamp` is the latest-at value across its filled columns, so most rows match the
///   predicate on a value pulled forward from the fixture's first chunk while sitting in a
///   chunk whose own `U` bounds a wrongly-eligible foreign pushdown would prune;
/// - a `T AND U` conjunction, where `T` wins and the `U` conjunct stays residual;
/// - a mixed-timeline `OR`, which no single selection timeline covers, so nothing is pushed;
/// - the same `U`-only predicate on the legacy `query_dataset` path, as a control.
pub async fn query_dataset_foreign_selection_timeline_matches_oracle(
    service: impl RerunCloudService,
) {
    let service = Arc::new(service);

    // Two segments whose frame sequences and timestamps interleave, so neither timeline's
    // ordering can stand in for the other's across segments either.
    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        TWO_TIMELINE_TUID_PREFIX,
        [
            LayerDefinition::two_timelines("two_timeline_segment1", 0, 0),
            LayerDefinition::two_timelines("two_timeline_segment2", 5, 15),
        ],
    );

    let dataset_name = "dataset";
    let dataset_entry = service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;
    // Keep the temp RRD files alive: the OSS server re-reads them while serving candidates.
    let _data_sources = data_sources_def;

    // The foreign-timeline pruning cases need each segment's temporal chunks to stay separate.
    // Were the server to merge them, a selection on `timestamp` would have nothing to prune and
    // those cases would pass vacuously.
    let expected_chunks: BTreeSet<ChunkId> = (0..2)
        .flat_map(|layer| two_timeline_chunk_ids(TWO_TIMELINE_TUID_PREFIX + layer))
        .collect();
    let selected_chunks = find_chunks_ids(
        service.as_ref(),
        dataset_name,
        range_request(
            TWO_TIMELINE_READER_INDEX,
            None,
            None,
            FillMode::None,
            SegmentSelection::All,
        ),
    )
    .await;
    assert_eq!(
        selected_chunks, expected_chunks,
        "each two-timeline segment must keep its three temporal chunks and one static chunk"
    );

    // Straddles the fixture's chunk bounds on `U` both ways: without fill it keeps rows from
    // one chunk per segment, and under fill it keeps the rows whose own `U` is far below it but
    // whose latest-at value is pulled up into it.
    let u_only = col(TWO_TIMELINE_FOREIGN_INDEX)
        .gt_eq(timestamp_ms(85))
        .and(col(TWO_TIMELINE_FOREIGN_INDEX).lt_eq(timestamp_ms(100)));
    let t_and_u = col(TWO_TIMELINE_READER_INDEX)
        .gt_eq(lit(40))
        .and(col(TWO_TIMELINE_FOREIGN_INDEX).lt_eq(timestamp_ms(55)));
    let mixed_or = col(TWO_TIMELINE_READER_INDEX)
        .lt_eq(lit(25))
        .or(col(TWO_TIMELINE_FOREIGN_INDEX).lt_eq(timestamp_ms(25)));

    let cases = [
        (
            "U-only, no fill",
            u_only.clone(),
            re_chunk_store::SparseFillStrategy::None,
            true,
        ),
        (
            "U-only, latest-at fill",
            u_only.clone(),
            re_chunk_store::SparseFillStrategy::LatestAtGlobal,
            true,
        ),
        (
            "T AND U",
            t_and_u,
            re_chunk_store::SparseFillStrategy::None,
            true,
        ),
        (
            "mixed-timeline OR",
            mixed_or,
            re_chunk_store::SparseFillStrategy::None,
            true,
        ),
        (
            "U-only, no fill, legacy path",
            u_only,
            re_chunk_store::SparseFillStrategy::None,
            false,
        ),
    ];

    for (case, filter, sparse_fill_strategy, supports_find_chunks) in cases {
        assert_filter_matches_oracle(
            &service,
            dataset_entry.details.id,
            sparse_fill_strategy,
            filter,
            supports_find_chunks,
            case,
        )
        .await;
    }
}

/// Records the difference between filtering chunks on `timestamp` and filtering completed rows.
///
/// At frame 30, one entity has `timestamp = 90 ms`. The `frame/only` entity has a value at the
/// same frame, but its temporal chunk has no `timestamp` timeline. Selecting chunks for
/// `85 ms <= timestamp <= 100 ms` excludes that chunk. The reader keeps frame 30 but returns
/// null for `frame/only`. Reading all chunks first and then filtering the rows retains its value.
///
/// The null is the known defect [RR-5885](https://linear.app/rerun/issue/RR-5885). The last
/// assertion records it, and must expect `[true]` once the defect is fixed.
pub async fn query_dataset_entity_without_filtered_timeline_reads_null(
    service: impl RerunCloudService,
) {
    const FRAME_ONLY_ENTITY_PATH: &str = "frame/only";
    const SEGMENT_ID: &str = "two_timeline_segment";

    let service = Arc::new(service);

    let data_sources_def = DataSourcesDefinition::new_with_tuid_prefix(
        41,
        [
            LayerDefinition::two_timelines(SEGMENT_ID, 0, 0),
            LayerDefinition::simple_with_time(
                SEGMENT_ID,
                &[FRAME_ONLY_ENTITY_PATH],
                0,
                re_log_types::TimeType::Sequence,
            )
            .layer_name("frame_only"),
        ],
    );

    let dataset_name = "entity_without_filtered_timeline";
    let dataset_entry = service.create_dataset_entry_with_name(dataset_name).await;
    service
        .register_with_dataset_name_blocking(dataset_name, data_sources_def.to_data_sources())
        .await;
    // Keep the temp RRD files alive: the OSS server re-reads them while serving candidates.
    let _data_sources = data_sources_def;

    let query = re_chunk_store::QueryExpression {
        view_contents: Some(
            [TWO_TIMELINE_ENTITY_PATH, FRAME_ONLY_ENTITY_PATH]
                .into_iter()
                .map(|path| (EntityPath::from(path), None))
                .collect(),
        ),
        filtered_index: Some(TWO_TIMELINE_READER_INDEX.into()),
        sparse_fill_strategy: re_chunk_store::SparseFillStrategy::None,
        ..Default::default()
    };

    // Keeps only frame 30, whose `timestamp` is 90 ms. `frame/only` logged a value at frame 30.
    let filter = col(TWO_TIMELINE_FOREIGN_INDEX)
        .gt_eq(timestamp_ms(85))
        .and(col(TWO_TIMELINE_FOREIGN_INDEX).lt_eq(timestamp_ms(100)));

    let unfiltered = collect_through_datafusion(
        two_timeline_provider(&service, dataset_entry.details.id, &query, true).await,
        None,
    )
    .await;
    let filtered = collect_through_datafusion(
        two_timeline_provider(&service, dataset_entry.details.id, &query, true).await,
        Some(filter.clone()),
    )
    .await;
    let oracle = filter_in_memory(&unfiltered, filter).await;

    let frame_only_points = |batches: &[RecordBatch]| -> Vec<bool> {
        let batch = concat_record_batches(batches);
        let schema = batch.schema();
        let column_name = format!(
            "/{FRAME_ONLY_ENTITY_PATH}:{}",
            MyPoints::descriptor_points().component
        );
        let column = batch.column_by_name(&column_name).unwrap_or_else(|| {
            panic!(
                "no column {column_name} in {:?}",
                schema
                    .fields()
                    .iter()
                    .map(|field| field.name())
                    .collect_vec()
            )
        });
        (0..column.len()).map(|row| column.is_valid(row)).collect()
    };

    assert_eq!(
        frame_only_points(&oracle),
        vec![true],
        "the oracle must keep one row, where `frame/only` has a value"
    );
    // TODO(RR-5885): expect `[true]` once selecting chunks on a foreign timeline keeps the
    // values of entities that never logged it. https://linear.app/rerun/issue/RR-5885
    assert_eq!(
        frame_only_points(&filtered),
        vec![false],
        "the filtered read keeps the same row, with `frame/only` null: this records the known \
         defect RR-5885 (https://linear.app/rerun/issue/RR-5885), not the intended behavior"
    );
}

/// Assert that reading the two-timeline fixture under `filter` returns exactly the rows an
/// unfiltered read of the same reader, filtered again in memory, produces.
///
/// Both sides run the whole DataFusion pipeline, so whatever the provider leaves residual is
/// still applied and only rows the pushdown *lost* can make the two differ.
async fn assert_filter_matches_oracle<T: RerunCloudService>(
    service: &Arc<T>,
    dataset_id: EntryId,
    sparse_fill_strategy: re_chunk_store::SparseFillStrategy,
    filter: Expr,
    supports_find_chunks: bool,
    case: &str,
) {
    let query = re_chunk_store::QueryExpression {
        view_contents: Some(
            std::iter::once((EntityPath::from(TWO_TIMELINE_ENTITY_PATH), None)).collect(),
        ),
        filtered_index: Some(TWO_TIMELINE_READER_INDEX.into()),
        sparse_fill_strategy,
        ..Default::default()
    };

    // A fresh provider (and client) per read, so neither read can observe the other's state.
    let unfiltered = collect_through_datafusion(
        two_timeline_provider(service, dataset_id, &query, supports_find_chunks).await,
        None,
    )
    .await;
    let filtered = collect_through_datafusion(
        two_timeline_provider(service, dataset_id, &query, supports_find_chunks).await,
        Some(filter.clone()),
    )
    .await;
    let oracle = filter_in_memory(&unfiltered, filter).await;

    let unfiltered_rows = total_rows(&unfiltered);
    let oracle_rows = total_rows(&oracle);
    assert!(
        0 < oracle_rows && oracle_rows < unfiltered_rows,
        "case {case}: the predicate must keep some rows and drop some, \
         otherwise the comparison below proves nothing; \
         kept {oracle_rows} of {unfiltered_rows} rows"
    );
    assert_eq!(
        total_rows(&filtered),
        oracle_rows,
        "case {case}: the filtered read returned a different number of rows than the oracle"
    );

    assert_eq!(
        format_rows(&filtered),
        format_rows(&oracle),
        "case {case}: the filtered read and the unfiltered-plus-filter oracle disagree"
    );
}

async fn two_timeline_provider<T: RerunCloudService>(
    service: &Arc<T>,
    dataset_id: EntryId,
    query: &re_chunk_store::QueryExpression,
    supports_find_chunks: bool,
) -> DataframeQueryTableProvider<TestClient<T>> {
    DataframeQueryTableProvider::new_from_client(
        TestClient::new(Arc::clone(service)),
        dataset_id,
        query,
        &[] as &[&str],
        None,
        None,
        None,
        Vec::new(),
        Arc::new(NoOpObjectStoreAuthenticator::default()),
    )
    .await
    .expect("build the two-timeline table provider")
    .with_find_chunks_support(supports_find_chunks)
}

/// Read `provider` through a full DataFusion plan, so a filter the provider reports as
/// `Inexact` is still applied to the rows it returns.
async fn collect_through_datafusion<T: DataframeClientAPI>(
    provider: DataframeQueryTableProvider<T>,
    filter: Option<Expr>,
) -> Vec<RecordBatch> {
    let ctx = SessionContext::default();
    let frame = ctx
        .read_table(Arc::new(provider))
        .expect("read the table provider");
    let frame = match filter {
        Some(filter) => frame.filter(filter).expect("apply the filter"),
        None => frame,
    };
    frame.collect().await.expect("collect the rows")
}

/// The oracle: apply `filter` to already-collected rows, with no server in the loop.
async fn filter_in_memory(batches: &[RecordBatch], filter: Expr) -> Vec<RecordBatch> {
    let schema = batches
        .first()
        .expect("the unfiltered read must return at least one batch")
        .schema();
    let table = MemTable::try_new(schema, vec![batches.to_vec()])
        .expect("build the in-memory oracle table");

    let ctx = SessionContext::default();
    ctx.read_table(Arc::new(table))
        .expect("read the oracle table")
        .filter(filter)
        .expect("apply the filter to the oracle")
        .collect()
        .await
        .expect("collect the oracle rows")
}

fn total_rows(batches: &[RecordBatch]) -> usize {
    batches.iter().map(|batch| batch.num_rows()).sum()
}

/// Render rows in a canonical order, so the comparison does not depend on partitioning.
fn format_rows(batches: &[RecordBatch]) -> String {
    concat_record_batches(batches)
        .horizontally_sorted()
        .auto_sort_rows()
        .expect("sort the rows canonically")
        .format_snapshot(false)
}
