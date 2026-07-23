use crate::RecordBatchTestExt as _;
use crate::tests::common::{
    DataSourcesDefinition, LayerDefinition, RerunCloudServiceExt as _, concat_record_batches,
};
use crate::utils::client::{TestClient, create_test_client};
use arrow::array::RecordBatch;
use datafusion::datasource::TableProvider as _;
use datafusion::execution::SessionState;
use datafusion::physical_plan::ExecutionPlanProperties as _;
use datafusion::prelude::{Expr, SessionConfig, SessionContext, col, lit};
use futures::{StreamExt as _, TryStreamExt as _};
use itertools::Itertools as _;
use re_datafusion::{DataframeClientAPI, DataframeQueryTableProvider};
use re_log_types::{EntityPath, EntryId};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use std::sync::Arc;

pub async fn query_dataset_simple_filter(service: impl RerunCloudService) {
    #![expect(unsafe_code)]
    let original_env = std::env::var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED").ok();

    // SAFETY:
    // This is simply a test
    unsafe { std::env::set_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED", "3") };

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

    // SAFETY:
    // This is simply a test
    unsafe {
        match original_env {
            Some(val) => std::env::set_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED", val),
            None => std::env::remove_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED"),
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
    #![expect(unsafe_code)]
    let original_env = std::env::var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED").ok();

    // SAFETY:
    // This is simply a test
    unsafe { std::env::set_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED", "3") };

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
    )
    .await
    .unwrap();
    query_dataset_count_and_snapshot(
        &fill_provider,
        range_filter,
        "frame_nr_range_fill_latest_at",
    )
    .await;

    // SAFETY:
    // This is simply a test
    unsafe {
        match original_env {
            Some(val) => std::env::set_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED", val),
            None => std::env::remove_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED"),
        }
    }
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
    #![expect(unsafe_code)]
    let original_env = std::env::var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED").ok();

    // SAFETY:
    // This is simply a test
    unsafe { std::env::set_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED", "3") };

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

    // SAFETY:
    // This is simply a test
    unsafe {
        match original_env {
            Some(val) => std::env::set_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED", val),
            None => std::env::remove_var("RERUN_CHUNK_MAX_ROWS_IF_UNSORTED"),
        }
    }
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
    let (rows_three_again, _) =
        scan_collect_rows(&service, dataset_entry.details.id, &query, filter_three).await;
    assert_eq!(
        rows_three, rows_three_again,
        "concurrent fan-out must produce identical rows across runs"
    );

    // 3. Dedup: dropping the redundant overlapping branch must not change rows.
    //    If dedup regressed, `C`'s chunks would be fetched twice and their rows
    //    would be duplicated, making `rows_three` strictly larger than `rows_two`.
    let (rows_two, requests_two) =
        scan_collect_rows(&service, dataset_entry.details.id, &query, filter_two).await;
    assert_eq!(
        requests_two, 2,
        "an OR of two ranges must fan out to exactly two query_dataset requests"
    );
    assert_eq!(
        rows_three, rows_two,
        "a redundant overlapping range must not add or duplicate rows"
    );

    insta::assert_snapshot!("or_of_ranges_fanout_data", rows_three);
}

/// Run [`DataframeQueryTableProvider::scan`] for `filter` on a fresh
/// [`TestClient`], returning the canonically-sorted row snapshot and the number
/// of `query_dataset` requests the scan emitted.
///
/// A fresh client per call keeps the recorded request count scoped to this one
/// scan.
async fn scan_collect_rows<T: RerunCloudService>(
    service: &Arc<T>,
    dataset_id: EntryId,
    query: &re_chunk_store::QueryExpression,
    filter: Expr,
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
    )
    .await
    .unwrap();

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

    let requests = client.query_dataset_requests.lock().len();
    (sorted.format_snapshot(false), requests)
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
